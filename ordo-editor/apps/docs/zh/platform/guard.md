# Agent 护栏 (`ordo guard`)

大模型是非确定性的——同一个问题问两遍，可能得到两个答案。用来起草文字没问题，但一旦 Agent 要跑 shell、改文件、调 API，这种不确定就变得危险。`ordo guard` 在你的编码 Agent 前面加一层**确定性决策层**：每一次工具调用都交给本地的 Ordo 规则裁决——**放行 / 拒绝 / 询问**。

它和随手写的 `if` 判断或手搓白名单的区别在于：策略本身就是一个**标准的 Ordo 项目**，所以你的护栏自带测试套件、可以 trace 调试，而且每一次决策都会写进审计日志。

一份策略，接到哪个 Agent 上就在哪个 Agent 上生效：

| Agent           | 钩子                    | 配置文件                  | 覆盖范围                                    |
| --------------- | ------------------------ | -------------------------- | -------------------------------------------- |
| **Claude Code** | `PreToolUse`              | `.claude/settings*.json`   | 所有工具（Bash、Read、Write、Edit、WebFetch…） |
| **Codex CLI**   | `PreToolUse`              | `.codex/hooks.json`        | 目前仅 Bash（上游限制）                       |
| **Cursor**      | `beforeShellExecution`    | `.cursor/hooks.json`       | 仅 shell 命令                                 |

Claude Code 和 Codex CLI 走的是同一套信封格式，同一条规则在两边行为一致。Cursor
只能看到 shell 命令，所以基于 `file_path`/`url` 的规则在 Cursor 上永远不会触发——
Cursor 事件的 `tool` 恒为 `"Bash"`。

## 安装（五分钟）

在你想加护栏的仓库里：

```bash
npx @ordo-engine/cli guard init                        # Claude Code（默认）
npx @ordo-engine/cli guard init --agent codex           # Codex CLI
npx @ordo-engine/cli guard init --agent cursor          # Cursor
npx @ordo-engine/cli guard init --agent claude,codex,cursor   # 一次接三个
```

`--agent` 可重复传（`--agent codex --agent cursor`）或逗号分隔；每个选中的 Agent
各自拿到自己的钩子命令和配置文件，但评估的是同一份
`.ordo-guard/rulesets/policy.json`。它做两件事：

1. 生成 `.ordo-guard/`——一个 Ordo 项目，包含 `rulesets/policy.json`、
   `tests/policy.json`、`facts.json` 和一份 `AGENTS.md`。只生成一次，各 Agent 共用。
2. 为每个选中的 Agent 注册钩子（具体文件见上表）。

重启对应的 Agent（Claude Code 可运行 `/hooks`）让它生效。此后每一次工具调用都会走你的策略：

```text
$ (Agent 尝试) rm -rf ./build
⛔ 已被策略拒绝：Destructive shell command blocked by policy [policy@1.0.0 · DENY]
```

默认策略会拦截破坏性 shell（`rm -rf`、`dd`、`mkfs`）和密钥访问（`.env`、`.pem`、
`id_rsa`、aws 凭证），在 `git push` / `npm publish` / 修改护栏本身之前询问，对只读
git 快速放行，其余一律交回 Agent 的正常权限流程。

::: tip 团队共享
默认注册用的是各 Agent 本地、git 忽略的配置文件里的绝对路径。若想给整个团队提交一份
可移植的钩子，加上 `--shared`——它会注册 `npx -y @ordo-engine/cli guard hook`
（Codex/Cursor 会带上对应的 `--agent` 后缀）到该 Agent 的共享/已提交配置里。只有
Claude Code 区分共享与本地文件（`.claude/settings.json` 对 `.claude/settings.local.json`）；
Codex CLI 和 Cursor 各自只有一个项目本地文件，但 `--shared` 仍会切换成可移植命令。
:::

::: warning Cursor 的决策信封
Cursor 的 `beforeShellExecution` 协议只定义了 `allow` / `deny` / `ask`，没有明确的
"不表态"语义——所以和 Claude Code / Codex CLI（未命中规则时 stdout 保持空、交回 Agent
自己的流程）不同，Cursor 上未命中任何策略规则会显式返回 `allow`，而不是保持静默。
:::

## 策略能看到的输入

钩子会把 Agent 的事件拍平成同一个输入对象——不管是哪个 Agent 触发的，形状都一样，
所以规则只需写一次，到哪都一致。在条件里直接引用这些字段：

| 字段              | 示例                | 说明                                          |
| ----------------- | ------------------- | ---------------------------------------------- |
| `tool`            | `"Bash"`、`"Edit"`  | 工具名——Cursor 事件恒为 `"Bash"`               |
| `command`         | `"git push origin"` | Bash——从 `tool_input` 提升上来                 |
| `file_path`       | `"src/main.rs"`     | Read/Write/Edit——已提升（仅 Claude Code/Codex）|
| `url`             | `"https://…"`       | WebFetch——已提升（仅 Claude Code/Codex）       |
| `cwd`             | `"/repo"`           | 工作目录                                        |
| `permission_mode` | `"default"`         | Claude Code/Codex 权限模式；Cursor 上不存在     |
| `session_id`      | `"c1a2…"`           | Cursor 的 `conversation_id` 也映射到这个字段    |
| `tool_input`      | `{ … }`             | 完整的嵌套工具输入                              |

`tool_input` 里的其他键也会被提升到顶层，所以出现新工具时无需改代码即可在条件里使用。

::: warning 缺失字段是宽松的
条件引用一个**不存在**的字段时结果为 `false`，所以基于 `command` 的规则对非 Bash 工具会
被安全跳过。注意取反：当 `command` 缺失时，`!(command contains 'x')` **也**是 false。建议先
用工具名兜底：`tool == 'Bash' && !(command contains 'x')`。
:::

## 编写规则

分支条件是朴素的表达式字符串，从上到下求值——首个匹配生效。终结节点的 code 映射到决策：
`DENY`、`ASK`、`ALLOW`，而 `PASS`（或任何其他 code）= 不表态。

```json
{
  "id": "gate-b0",
  "label": "拦截 terraform destroy",
  "condition": "tool == 'Bash' && command contains 'terraform destroy'",
  "nextStepId": "deny_infra"
}
```

表达式语言支持 `== != > >= < <=`、`&&` `||` `!`、`in`、`contains`，以及
`starts_with(s, prefix)`、`ends_with(s, suffix)`、`regex_match(pattern, s)` 等函数。

::: warning `regex_match` 的参数顺序
**模式在前**：`regex_match('rm\\s+-rf', command)`，不要写反。
:::

展示给 Agent 的决策原因来自命中的终结节点 `message`（或你设置的 `reason` 输出字段）。

## 测试你的护栏

因为策略是一个真正的 Ordo 项目，在 `tests/policy.json` 里加一条用例：

```json
{
  "name": "拦截 terraform destroy",
  "input": { "tool": "Bash", "command": "terraform destroy" },
  "expect": { "code": "DENY" }
}
```

然后运行：

```bash
ordo guard test
# --- PASS: blocks rm -rf (0.10ms)
# --- PASS: asks before git push (0.09ms)
# …
```

逐步调试某个事件的走向：

```bash
cd .ordo-guard
ordo trace policy --input '{"tool":"Bash","command":"git push"}'
```

## 审计日志

每一次决策都会追加到 `.ordo-guard/log.jsonl`（git 忽略）：

```bash
ordo guard log --tail 20
ordo guard log --json | jq 'select(.decision=="deny")'
```

每条记录包含时间戳、会话 id、工具、决策、原因、耗时，以及该次调用的一行摘要。

## 默认失败即放行

如果护栏自己出了问题——策略缺失、规则编译不过、事件格式错误——钩子会**失败即放行**：
在 stderr 打一行警告，stdout 保持静默，工具调用照常走 Agent 的正常流程。（在 Cursor 上，
"静默"意味着显式返回 `allow` 而非空 stdout——见前文提示。）坏掉的护栏绝不该卡死你的 Agent。
在注册的命令里加 `--fail-closed` 可反转此行为，改为内部出错时拒绝。

## 局限

Guard 是**纵深防御，不是沙箱**。它看到的是工具*调用*，不是其副作用：一条"修改 `.ordo-guard/`
之前询问"的规则，拦不住用 `bash sed -i` 做同样修改的调用。请把它和 Agent 自身的权限系统
叠加使用，不要把它当成对抗恶意进程的安全边界。

各 Agent 目前的已知缺口（都是上游限制，非 `ordo guard` 能绕开的）：Codex CLI 的
`PreToolUse` 目前只对 Bash 触发（Read/Write/Edit/MCP 调用尚不触发）；Cursor 的
`beforeShellExecution` 只能看到 shell 命令（完全没有文件编辑钩子），所以基于文件路径的
规则在 Cursor 上无法触达。

# Agent 护栏 (`ordo guard`)

大模型是非确定性的——同一个问题问两遍，可能得到两个答案。用来起草文字没问题，但一旦 Agent 要跑 shell、改文件、调 API，这种不确定就变得危险。`ordo guard` 在你的编码 Agent 前面加一层**确定性决策层**：每一次工具调用都交给本地的 Ordo 规则裁决——**放行 / 拒绝 / 询问**。

它和随手写的 `if` 判断或手搓白名单的区别在于：策略本身就是一个**标准的 Ordo 项目**，所以你的护栏自带测试套件、可以 trace 调试，而且每一次决策都会写进审计日志。

## 安装（五分钟）

在你想加护栏的仓库里：

```bash
npx @ordo-engine/cli guard init
```

它做两件事：

1. 生成 `.ordo-guard/`——一个 Ordo 项目，包含 `rulesets/policy.json`、
   `tests/policy.json`、`facts.json` 和一份 `AGENTS.md`。
2. 在 `.claude/settings.local.json` 里注册一个 Claude Code **PreToolUse** 钩子，
   指向 `ordo guard hook`。

重启 Claude Code（或运行 `/hooks`）让它生效。此后每一次工具调用都会走你的策略：

```text
$ (Agent 尝试) rm -rf ./build
⛔ 已被策略拒绝：Destructive shell command blocked by policy [policy@1.0.0 · DENY]
```

默认策略会拦截破坏性 shell（`rm -rf`、`dd`、`mkfs`）和密钥访问（`.env`、`.pem`、
`id_rsa`、aws 凭证），在 `git push` / `npm publish` / 修改护栏本身之前询问，对只读
git 快速放行，其余一律交回 Claude Code 的正常权限流程。

::: tip 团队共享
默认注册用的是 git 忽略的 `settings.local.json` 里的绝对路径。若想给整个团队提交一份
可移植的钩子，运行 `ordo guard init --shared`——它会在 `.claude/settings.json` 里注册
`npx -y @ordo-engine/cli guard hook`。
:::

## 策略能看到的输入

钩子会把 PreToolUse 事件拍平成一个输入对象。在条件里直接引用这些字段：

| 字段              | 示例                | 说明                           |
| ----------------- | ------------------- | ------------------------------ |
| `tool`            | `"Bash"`、`"Edit"`  | 工具名                         |
| `command`         | `"git push origin"` | Bash——从 `tool_input` 提升上来 |
| `file_path`       | `"src/main.rs"`     | Read/Write/Edit——已提升        |
| `url`             | `"https://…"`       | WebFetch——已提升               |
| `cwd`             | `"/repo"`           | 工作目录                       |
| `permission_mode` | `"default"`         | Claude Code 权限模式           |
| `tool_input`      | `{ … }`             | 完整的嵌套工具输入             |

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
在 stderr 打一行警告，stdout 保持静默，工具调用照常走 Claude Code 的正常流程。坏掉的护栏
绝不该卡死你的 Agent。在注册的命令里加 `--fail-closed` 可反转此行为，改为内部出错时拒绝。

## 局限

Guard 是**纵深防御，不是沙箱**。它看到的是工具*调用*，不是其副作用：一条"修改 `.ordo-guard/`
之前询问"的规则，拦不住用 `bash sed -i` 做同样修改的调用。请把它和 Claude Code 自身的权限
叠加使用，不要把它当成对抗恶意进程的安全边界。

当前范围：PreToolUse 事件、Claude Code。决策内核与具体 Agent 无关，因此支持其他 Agent 是后续可做的事。

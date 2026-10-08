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

重启对应的 Agent（Claude Code 可运行 `/hooks`）让它生效，然后确认整条链路是通的：

```bash
ordo guard doctor
# ✔ policy evaluates: policy@1.1.0
# ✔ policy tests: 24 passed
# ✔ Claude Code hook (.claude/settings.local.json): answers `rm -rf /` with deny
```

此后每一次工具调用都会走你的策略：

```text
$ (Agent 尝试) rm -r -f ./build
⛔ 已被策略拒绝：Destructive shell command blocked by policy [policy@1.1.0 · DENY]
```

默认策略会拦截破坏性 shell（递归 `rm`、`find -delete`、`dd`、`mkfs`、`shred`）和密钥访问
（`.env`、`.pem`、`id_rsa`、aws 凭证，无论走文件工具、`Grep` 还是 shell 参数），在
`git push` / `npm publish` / `git reset --hard` / `git clean`、以及修改护栏或 Agent 钩子配置之前询问，
对单条只读 git 命令快速放行，其余一律交回 Agent 的正常权限流程。shell 规则匹配的是*解析后*的命令
（见 [Shell 命令](#shell-命令)），所以 `rm -rf`、`rm -r -f`、`/bin/rm -Rf`、
`sudo env X=1 rm --recursive` 和 `bash -c 'rm -rf x'` 命中的是同一条规则。

::: tip 通过 npx 运行
`npx` 是从它的包缓存里运行二进制的，`npm cache clean` 会把它删掉，而 Agent 遇到程序不存在的钩子会静默跳过。
所以 `guard init` 从 npx 缓存运行时，会把二进制复制到 `~/.ordo/bin/ordo` 并注册这个路径。升级后重新运行一次
`guard init` 即可刷新。
:::

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
| `task_context`    | `"active"`          | 始终存在，见[任务上下文](#任务上下文)            |
| `task`、`rel_path` | `{ … }`、`"src/a.ts"` | 仅在任务上下文生效时出现                  |
| `programs`、`subcommands`、`argv`、`commands`、`words`、`shell_parse` | | 仅 Bash，见 [Shell 命令](#shell-命令) |

`tool_input` 里的其他键也会被提升到顶层，所以出现新工具时无需改代码即可在条件里使用。

::: warning 缺失字段是宽松的
条件引用一个**不存在**的字段时结果为 `false`，所以基于 `command` 的规则对非 Bash 工具会
被安全跳过。注意取反：当 `command` 缺失时，`!(command contains 'x')` **也**是 false。建议先
用工具名兜底：`tool == 'Bash' && !(command contains 'x')`。
:::

## Shell 命令

对 `command` 做子串匹配很容易绕过：`command contains 'rm -rf'` 拦不住 `rm -r -f`、`rm  -rf`（两个空格）和
`rm -Rf`。所以对每个 `Bash` 调用，钩子还会像 POSIX shell 一样解析 `command`——引号和转义、`&&` `||` `;` `|` `&`、
子 shell、`$(…)` 和反引号、`bash -c '…'` 和 `eval` 的内容、heredoc（正文是数据，不是命令）——剥掉 `sudo`、
`doas`、`env`、`nohup`、`nice`、`timeout`、`xargs`、`command` 等包装，然后加上这些字段：

| 字段          | `sudo git -C web push && rm -rf /tmp/x` 的结果 | 用法                              |
| ------------- | ---------------------------------------------- | --------------------------------- |
| `programs`    | `["sudo", "git", "rm"]`                        | `'rm' in programs`                |
| `subcommands` | `["git push", "rm /tmp/x"]`（程序 + 第一个非选项参数，会跳过 `git -C dir` 这类选项） | `'git push' in subcommands` |
| `argv`        | `{"git": ["-C", "web", "push"], "rm": ["-rf", "-r", "-f", "/tmp/x"], …}`，短选项组合也会拆开 | `'-r' in argv.rm` |
| `commands`    | `["sudo git -C web push", "rm -rf /tmp/x"]`    | `len(commands) == 1`              |
| `words`       | 所有单个 token 的参数和重定向目标（commit message 这类自由文本不算） | `regex_match('[.]env', join(words, ' '))` |
| `shell_parse` | `"ok"`；引号不配对或嵌套过深时为 `"error"`      | `shell_parse == 'error'`          |

```json
{
  "id": "gate-tf",
  "label": "拦截 terraform destroy",
  "condition": "tool == 'Bash' && 'terraform destroy' in subcommands",
  "nextStepId": "deny_infra"
}
```

这是静态分析，不会执行：变量、alias 和 shell 函数它都看不到，所以它是提高门槛，而不是堵死所有路。

::: warning 一个缺失字段会让整个条件为假
`rm` 执行了的话 `'-r' in argv.rm` 没问题；但如果没有 `rm`，`argv.rm` 就不存在，*整个*条件都是假，
连本来能命中的 `||` 分支也一样。每次查找前先守卫（`'rm' in programs && '-r' in argv.rm`），
读取不同字段的备选条件请拆成不同的分支。
:::

## 编写规则

分支条件是朴素的表达式字符串，从上到下求值——首个匹配生效。终结节点的 code 映射到决策：
`DENY`、`ASK`、`ALLOW`，而 `PASS`（或任何其他 code）= 不表态。

```json
{
  "id": "gate-migrations",
  "label": "修改迁移文件前确认",
  "condition": "tool in ['Write', 'Edit'] && file_path contains 'migrations/'",
  "nextStepId": "ask_migration"
}
```

表达式语言支持 `== != > >= < <=`、`&&` `||` `!`、`in`、`contains`，以及
`starts_with(s, prefix)`、`ends_with(s, suffix)`、`regex_match(pattern, s)`、`glob_match(模式或模式数组, s)` 等函数。

::: warning `regex_match` 的参数顺序和反斜杠
**模式在前**：`regex_match('[.]pem$', file_path)`，不要写反。表达式字符串里的反斜杠是转义符
（`'\s'` 传到正则时只剩 `s`），所以请用 `[.]` 和空格，而不是 `\.` 和 `\s`。
:::

展示给 Agent 的决策原因来自命中的终结节点 `message`（或你设置的 `reason` 输出字段）。

## 任务上下文

工具调用事件里没有“当前任务”的信息，所以策略能写“永远不准 `terraform destroy`”，却写不出
“这个任务只许改 `src/auth/`”。要按任务限定范围，就写一个 `.ordo-guard/context.json`
（手写或由任务规划工具生成），再用一份手写策略去读它：

```json
{
  "root": "/abs/path/to/repo",
  "session_id": "可选：只对这个 Agent 会话生效",
  "expires_at": "2026-10-08T00:00:00Z",
  "task": { "id": "login-signup", "touches": ["src/auth/**", "db/schema.sql"] }
}
```

`task` 原样透传，策略需要什么就放什么。只有和事件绑定上时上下文才生效：

| 检查         | 规则                                                       |
| ------------ | ----------------------------------------------------------- |
| `root`       | 事件的 `cwd` 必须在它之内（默认：`.ordo-guard/` 所在的仓库）   |
| `session_id` | 若设置，必须等于事件的会话 id                                |
| `expires_at` | 若设置（RFC 3339），必须尚未过期                             |

生效时，输入里会多出 `task` 和 `rel_path`：被编辑文件（`file_path` / `notebook_path`）
相对 `root` 的路径，用 `/` 分隔，`.` 和 `..` 已解析；root 之外的路径会是 `../…`。
输入里**始终**带有 `task_context`，由策略决定“没有可用任务”时怎么办：

| `task_context` | 含义                                       |
| -------------- | ------------------------------------------ |
| `active`       | 已生效：`task` 和 `rel_path` 已设置         |
| `absent`       | 没有 `context.json`                        |
| `mismatch`     | `cwd` 不在 `root` 内，或会话不匹配          |
| `expired`      | 已过 `expires_at`                          |
| `invalid`      | 无法读取或格式错误（stderr 会有警告）       |

把范围规则放在基础规则**之后**，这样任务内的 `rm -rf`、读取密钥依然会被拒绝：

```json
{
  "id": "gate-scope",
  "label": "限定在任务范围内",
  "condition": "task_context == 'active' && tool in ['Write', 'Edit'] && !glob_match(task.touches, rel_path)",
  "nextStepId": "ask_scope"
}
```

`glob_match` 接受单个模式或模式数组（任一匹配即为 true）。它的 `*` 也会匹配 `/`，
所以 `src/auth/*` 同样覆盖子目录；避免以通配符开头的模式（`**`、`*/…`），它们也会匹配
root 之外的 `../` 路径。

在上下文下产生的每条审计日志都会记录 `task_context`、`task_id` 和 `context_hash`
（对应 `context.json` 的 `sha256:`），因此每个决策都能追溯到当时生效的任务定义。
和 guard 的其他部分一样，这里检查的是工具*调用*：只比较路径字符串、不解析符号链接，
`Bash` 命令仍然可以写到任何地方。如果任务需要，给 `Bash` 配一条 `ASK` 规则。

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

`ordo guard test` 会像线上钩子一样从 `command` 推导出 [shell 字段](#shell-命令)，所以用例只需写 `tool` 和 `command`。

查看线上钩子对某个事件的回答：

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"git -C web push"}}' | ordo guard hook
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

失败即放行，或者钩子程序已经不存在（所有 Agent 都会静默跳过），都意味着你可能根本没发现护栏已经失效。
`ordo guard doctor` 会逐项检查：策略能求值、测试通过、钩子已注册、钩子程序存在，并且按 Agent 的方式实际运行
每个已注册的钩子，确认它对 `rm -rf /` 的回答。护栏没在保护仓库时它返回非零，所以也可以放进 CI 或 pre-commit。

## 局限

Guard 是**纵深防御，不是沙箱**。它看到的是工具*调用*，不是其副作用，shell 解析也看不穿变量和脚本：
默认策略会在 `sed -i … .ordo-guard/…` 之前询问，但拦不住一个修改同一文件的脚本。请把它和 Agent 自身的权限系统
叠加使用，不要把它当成对抗恶意进程的安全边界。

各 Agent 目前的已知缺口（都是上游限制，非 `ordo guard` 能绕开的）：Codex CLI 的
`PreToolUse` 目前只对 Bash 触发（Read/Write/Edit/MCP 调用尚不触发）；Cursor 的
`beforeShellExecution` 只能看到 shell 命令（完全没有文件编辑钩子），所以基于文件路径的
规则在 Cursor 上无法触达。

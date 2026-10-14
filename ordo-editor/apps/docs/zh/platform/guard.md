# Guard（`ordo guard`）

`ordo guard` 在编码 Agent 每次调用工具前，在本地跑一条 Ordo 规则。规则给出放行、拒绝或询问，同样的调用永远得到同样的回答。策略就是一个普通的 Ordo 项目，所以有测试、有追踪，每个决定都会记进日志。

| Agent       | Hook                   | 配置文件                 | 能看到                                   |
| ----------- | ---------------------- | ------------------------ | ---------------------------------------- |
| Claude Code | `PreToolUse`           | `.claude/settings*.json` | 所有工具（Bash、Read、Write、Edit、WebFetch 等） |
| Codex CLI   | `PreToolUse`           | `.codex/hooks.json`      | 目前只有 Bash（上游限制）                |
| Cursor      | `beforeShellExecution` | `.cursor/hooks.json`     | 只有 shell 命令                          |

Claude Code 和 Codex CLI 发来的事件格式相同，所以同一条规则在两者上表现一致。Cursor 只能看到 shell 命令，基于 `file_path` 或 `url` 的规则在它上面不会触发。

## 安装

在要保护的仓库里运行：

```bash
npx @ordo-engine/cli guard init                         # Claude Code
npx @ordo-engine/cli guard init --agent codex            # Codex CLI
npx @ordo-engine/cli guard init --agent claude,codex,cursor
```

它会创建 `.ordo-guard/`（策略在 `rulesets/policy.json`，还有测试、`facts.json`、`AGENTS.md`），并为每个 Agent 注册 hook。所有 Agent 共用一份策略。

重启 Agent（Claude Code 里运行 `/hooks` 就行），然后检查：

```bash
ordo guard doctor
# ✔ policy evaluates: policy@1.1.0
# ✔ policy tests: 24 passed
# ✔ Claude Code hook (.claude/settings.local.json): answers `rm -rf /` with deny
```

```text
$ (agent 尝试) rm -r -f ./build
⛔ Denied by policy: Destructive shell command blocked by policy [policy@1.1.0 · DENY]
```

默认策略：

- 拒绝破坏性的 shell 命令（递归 `rm`、`find -delete`、`dd`、`mkfs`、`shred`），
- 拒绝读取密钥（`.env`、`.pem`、`id_rsa`、AWS 凭证），不论通过文件工具、`Grep` 还是 shell 参数，
- 在 `git push`、`npm publish`、`git reset --hard`、`git clean` 之前，以及修改 guard 或 Agent 的 hook 配置之前询问，
- 放行单条只读的 git 命令，
- 其他的交给 Agent 自己的权限流程。

默认用绝对路径把 hook 注册在 git 忽略的本地配置里。通过 `npx` 运行 `guard init` 时，它会先把二进制复制到 `~/.ordo/bin/ordo`，因为程序不见了的 hook 会被 Agent 静默跳过。升级后请再运行一次 `guard init`。要提交一份全团队都能用的 hook，加 `--shared`：它会在共享配置里注册 `npx -y @ordo-engine/cli guard hook`。

## 策略看到的输入

hook 把 Agent 的事件整理成一个输入对象，不论来自哪个 Agent，格式都一样：

| 字段              | 例子                | 说明 |
| ----------------- | ------------------- | ---- |
| `tool`            | `"Bash"`、`"Edit"`  | Cursor 上总是 `"Bash"` |
| `command`         | `"git push origin"` | Bash |
| `file_path`       | `"src/main.rs"`     | Read、Write、Edit（Cursor 没有） |
| `url`             | `"https://…"`       | WebFetch（Cursor 没有） |
| `cwd`             | `"/repo"`           | |
| `permission_mode` | `"default"`         | Cursor 没有 |
| `session_id`      | `"c1a2…"`           | |
| `tool_input`      | `{ … }`             | 完整的工具输入，其中其他字段也会复制到顶层 |
| `task_context`、`task`、`rel_path` | | 见[任务范围](#任务范围) |
| `programs`、`subcommands`、`argv`、`commands`、`words`、`shell_parse` | | Bash，见下文 |

引用不存在的字段时条件为假。所以基于 `command` 的规则对其他工具会被跳过；但没有 `command` 时 `!(command contains 'x')` 也是假。先判断工具：`tool == 'Bash' && !(command contains 'x')`。

## Shell 命令

按文本匹配 `command` 很容易绕过：`rm -r -f`、`rm  -rf`、`rm -Rf` 都匹配不到 `'rm -rf'`。所以 hook 会像 shell 一样解析每条 Bash 命令：引号、`&&` `||` `;` `|`、子 shell、`$(…)`、`bash -c '…'`、`eval`、heredoc。它会穿过 `sudo`、`env`、`nohup`、`timeout`、`xargs` 这类包装，并加上这些字段：

| 字段          | 对 `sudo git -C web push && rm -rf /tmp/x` | 用法 |
| ------------- | ------------------------------------------ | ---- |
| `programs`    | `["sudo", "git", "rm"]`                    | `'rm' in programs` |
| `subcommands` | `["git push", "rm /tmp/x"]`                | `'git push' in subcommands` |
| `argv`        | `{"git": ["-C", "web", "push"], "rm": ["-rf", "-r", "-f", "/tmp/x"]}` | `'-r' in argv.rm` |
| `commands`    | `["sudo git -C web push", "rm -rf /tmp/x"]` | `len(commands) == 1` |
| `words`       | 单个 token 的参数和重定向目标              | `regex_match('[.]env', join(words, ' '))` |
| `shell_parse` | `"ok"`，引号不配对时是 `"error"`           | `shell_parse == 'error'` |

命令只被分析，不会被执行。变量、别名和函数对解析器不可见，所以这让绕过更难，但不能杜绝。

没有运行 `rm` 时 `argv.rm` 不存在，整个条件都会为假，`||` 后面的部分也一样。请写成 `'rm' in programs && '-r' in argv.rm`，读不同字段的几种情况放进不同的分支。

## 写规则

策略是 Studio 文件格式的规则集。分支从上往下检查，第一个命中的生效。终止步骤的结果码就是决定：`DENY`、`ASK`、`ALLOW`，`PASS`（或其他任何码）表示不表态。给 Agent 看的原因来自终止步骤的 `message`，或者 `reason` 输出字段。

```json
{
  "id": "gate-migrations",
  "label": "confirm migration edits",
  "condition": "tool in ['Write', 'Edit'] && file_path contains 'migrations/'",
  "nextStepId": "ask_migration"
}
```

条件就是普通的[表达式](/zh/guide/expression-syntax)。这里常用的函数有 `starts_with`、`ends_with`、`regex_match(pattern, s)` 和 `glob_match(patterns, s)`。`regex_match` 的模式写在前面。表达式字符串里的反斜杠是转义符（`'\s'` 传到正则时是 `s`），所以用 `[.]` 和普通空格，不要用 `\.` 和 `\s`。

## 任务范围

工具调用本身不带"Agent 在做什么任务"的信息。要把一个任务限制在某些文件内，写 `.ordo-guard/context.json`：

```json
{
  "root": "/abs/path/to/repo",
  "session_id": "可选：只对这个 Agent 会话生效",
  "expires_at": "2026-10-08T00:00:00Z",
  "task": { "id": "login-signup", "touches": ["src/auth/**", "db/schema.sql"] }
}
```

只有事件的 `cwd` 在 `root` 里、会话匹配（如果设置了）、还没过期时，上下文才生效。这时输入里会多出 `task`（原样传入）和 `rel_path`（被编辑文件相对 `root` 的路径）。`task_context` 总会设置：`active`、`absent`、`mismatch`、`expired` 或 `invalid`。

把范围规则放在基础规则之后，这样任务内 `rm -rf` 和读取密钥依然会被拒绝：

```json
{
  "id": "gate-scope",
  "label": "stay inside the task scope",
  "condition": "task_context == 'active' && tool in ['Write', 'Edit'] && !glob_match(task.touches, rel_path)",
  "nextStepId": "ask_scope"
}
```

`glob_match` 的 `*` 也会匹配 `/`。不要用以通配符开头的模式，它们也会匹配 root 之外的 `../` 路径。检查只比较路径字符串，不跟随符号链接，而 Bash 命令仍然可以写任何地方。对任务来说如果这一点重要，就对 Bash 加一条 `ASK`。

## 测试策略

在 `.ordo-guard/tests/policy.json` 里加一个用例：

```json
{
  "name": "blocks terraform destroy",
  "input": { "tool": "Bash", "command": "terraform destroy" },
  "expect": { "code": "DENY" }
}
```

```bash
ordo guard test
```

`guard test` 和 hook 一样从 `command` 算出 shell 字段，所以用例只需要 `tool` 和 `command`。想看 hook 对某个事件怎么回答：

```bash
echo '{"tool_name":"Bash","tool_input":{"command":"git -C web push"}}' | ordo guard hook
```

## 日志

每个决定都追加到 `.ordo-guard/log.jsonl`（已被 git 忽略），记录时间、会话、工具、决定、原因和一行摘要。在任务上下文下做的决定还会记录任务 ID 和 `context.json` 的哈希。

```bash
ordo guard log --tail 20
ordo guard log --json | jq 'select(.decision=="deny")'
```

## guard 自己出错时

策略缺失、编译不过或事件格式不对时，hook 在 stderr 打印警告，让这次调用走 Agent 自己的流程。坏掉的 guard 不应该卡住你的 Agent。注册 hook 时加 `--fail-closed` 可以改成拒绝。

也正因为这样，guard 可能失效了你却没发现。`ordo guard doctor` 会逐项检查：策略能运行、测试通过、每个 hook 都已注册、程序存在，并且对示例 `rm -rf /` 给出正确回答。guard 没有在保护仓库时它会以非零退出，可以放进 CI 或 pre-commit。

`guard init` 不会覆盖已有的 `.ordo-guard/`。要把旧的默认策略换成新的，运行 `ordo guard upgrade`（`--dry-run` 先预览）。你改过的文件会保留，除非加 `--force`，这时旧文件存为 `<file>.bak`。

## 限制

Guard 是多加的一层防护，不是沙箱。它看到的是工具调用，不是调用产生的效果，也看不进变量和脚本。请和 Agent 自己的权限设置一起用，不要指望它挡住一个存心绕过它的进程。

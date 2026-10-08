# MCP 服务（`ordo mcp`）

`ordo mcp` 把 Ordo 作为基于 stdio 的 [Model Context Protocol](https://modelcontextprotocol.io) 服务运行。编码 agent（Claude Code、Cursor、Windsurf 等）接入后可以使用 Ordo 的工具，在编辑器里读、写、校验、测试和发布决策规则。

## 接入

```bash
# Claude Code
claude mcp add ordo -- ordo mcp
```

其他客户端：添加一个 stdio MCP 服务，命令为 `ordo mcp`，在决策项目目录内运行（即 [`ordo init`](/zh/platform/cli) 生成的文件夹）。

## 工具

服务提供九个工具。读、写和检查类工具都在本地项目文件和内嵌引擎上运行，离线且即时返回；只有 `publish` 会连接平台。

| 工具          | 作用                              |
| ------------- | --------------------------------- |
| `list_files`  | 列出项目文件                      |
| `read_file`   | 读文件                            |
| `grep`        | 在文件里搜子串                    |
| `write_file`  | 新建/覆盖文件                     |
| `delete_file` | 删除 ruleset/tests/contracts 文件 |
| `validate`    | 编译规则，结构化报错               |
| `run_tests`   | 跑规则的测试用例                  |
| `trace`       | 对某输入执行并返回逐步路径        |
| `publish`     | 把规则发布到某环境                |

## 安全

文件都在本地，并由 git 管理，所以文件编辑可以撤销，默认允许。高风险操作需要显式开启：

```bash
ordo mcp --allow-publish     # 允许 publish 工具
ordo mcp --allow-delete      # 允许删除 ruleset 文件
```

不加 `--allow-publish` 时，`publish` 工具返回“已拦截”的结果，不会真正发布。agent 可以提出发布，最终由人决定。

## 典型流程

1. 你对 agent 说：_“加一条规则：金额 ≤ 10000 就通过，否则拒绝。”_
2. agent 用 `list_files` / `read_file` 了解项目结构，再用 `write_file` 添加 `rulesets/loan-approval.json`。
3. agent 调用 `validate` 和 `run_tests`，修复失败项。
4. agent 调用 `trace`，确认样例输入走的是预期路径。
5. 如果开启了 `--allow-publish`，agent 可以调用 `publish`；否则交给你发布。

`validate`/`test`/`trace` 离线运行，耗时在一秒以内，所以 agent 修改和检查的循环很快，结果也和平台一致（概念的物化方式相同）。

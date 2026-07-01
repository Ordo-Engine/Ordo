# MCP 服务（`ordo mcp`）

`ordo mcp` 把 Ordo 跑成一个基于 stdio 的 [Model Context Protocol](https://modelcontextprotocol.io) 服务,让编码 agent(Claude Code、Cursor、Windsurf……)原生拿到 Ordo 的工具——不离开编辑器就能替你读、写、校验、测试、发布决策规则。

## 接入

```bash
# Claude Code
claude mcp add ordo -- ordo mcp
```

其它客户端:添加一个 stdio MCP 服务,命令为 `ordo mcp`,在一个决策项目目录内运行(即 [`ordo init`](/zh/platform/cli) 生成的文件夹)。

## 工具

服务暴露九个工具。读/写/检查类工具都在**本地项目文件 + 内嵌引擎**上运行——离线、即时;只有 `publish` 会连平台。

| 工具          | 作用                              |
| ------------- | --------------------------------- |
| `list_files`  | 列出项目文件                      |
| `read_file`   | 读文件                            |
| `grep`        | 在文件里搜子串                    |
| `write_file`  | 新建/覆盖文件                     |
| `delete_file` | 删除 ruleset/tests/contracts 文件 |
| `validate`    | 编译规则,结构化报错               |
| `run_tests`   | 跑规则的测试用例                  |
| `trace`       | 对某输入执行并返回逐步路径        |
| `publish`     | 把规则发布到某环境                |

## 安全

服务本地优先、git 托底,所以文件编辑可逆、默认放行。高危动作用 flag 门控:

```bash
ordo mcp --allow-publish     # 允许 publish 工具
ordo mcp --allow-delete      # 允许删除 ruleset 文件
```

不加 `--allow-publish` 时,`publish` 工具返回"被拦截"的结果而不真发布——agent 可以提议发布,但人始终掌控。

## 典型流程

1. 你对 agent 说:_"加一条规则:金额 ≤ 10000 就通过,否则拒绝。"_
2. agent 用 `list_files` / `read_file` 摸清项目,再用 `write_file` 加 `rulesets/loan-approval.json`。
3. 它调 `validate` 和 `run_tests`,修掉失败项。
4. 它调 `trace` 确认样例输入走的是预期路径。
5. 配了 `--allow-publish` 就能 `publish`——否则交回给你。

因为 `validate`/`test`/`trace` 离线且亚秒,agent 的 改→查 循环很紧凑,而且结果和平台一致(概念物化方式相同)。

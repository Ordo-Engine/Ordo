# 命令行（`ordo`）

`ordo` 命令行把规则当作文件，放在你提交到 git 的目录里。它在本地检查、测试规则，也负责和平台同步。

## 安装

```bash
npm install -g @ordo-engine/cli    # 不想安装就用：npx @ordo-engine/cli
ordo --help
```

npm 包会下载适合 Linux、macOS 或 Windows 的预编译二进制。想自己构建：`cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli`。

每个命令都支持 `--json`，输出机器可读的结果。

## 磁盘上的项目

`ordo init my-rules` 会生成：

```text
ordo.yaml              项目设置和平台关联
rulesets/<name>.json   规则集
tests/<name>.json      规则集的测试用例
facts.json             事实目录
concepts.json          概念（基于事实的命名表达式）
contracts/<name>.json  决策契约，添加后才有
AGENTS.md              给编码 Agent 的说明
```

## 命令

| 命令 | 作用 |
| --- | --- |
| `init [dir]` | 新建项目 |
| `new ruleset\|fact\|concept <name>` | 添加规则集（带一个空测试文件）、事实或概念 |
| `validate [name]` | 编译规则集，报告错误 |
| `test [name]` | 跑测试用例。单个文件用 `--rule` 和 `--tests` |
| `trace <name> --input <json>` | 跑一条输入，打印执行路径 |
| `impact <name>` | 列出这次改动影响的决策，见[测试与影响](/zh/guide/testing) |
| `replay <file>` | 用当前规则重跑捕获的决策 |
| `decisions <path>` | 汇总网关的决策日志，见[决策模型网关](./decision-gateway) |
| `exec --rule <file> --input <json>` | 在项目之外跑单个规则文件 |
| `eval <expr> --input <json>` | 计算一个表达式 |
| `fmt`、`lint` | 格式化规则文件；检查图结构和风格 |
| `guard ...` | Agent 护栏，见 [Guard](./guard) |
| `mcp` | 作为 MCP 服务给编码 Agent 用，见 [MCP 服务](./mcp) |
| `login`、`whoami`、`link` | 登录平台，把目录关联到项目 |
| `pull`、`push`、`diff` | 和平台上的草稿同步文件 |
| `publish <name> --env <env>`、`deployments` | 发布规则集，查看部署情况 |
| `completions <shell>` | bash、zsh、fish、powershell、elvish 的补全脚本 |

`validate`、`test`、`trace`、`impact` 和 `replay` 都用内置引擎离线运行。概念的计算方式和平台一样，所以本地结果和线上一致。

## 和平台同步

```bash
ordo login
ordo link --org <org> --project <project>
ordo pull                                   # 规则集、目录和测试
# 修改文件
ordo push                                   # 规则集、事实、概念、测试和契约
ordo publish loan-approval --env staging
ordo deployments
ordo diff                                   # 本地文件和平台草稿的差异
```

`push` 默认全部上传，加 `--rulesets-only` 只传规则集。如果平台上有更新的修改，它会让你先 `ordo pull`。

令牌和 API 地址保存在 `~/.ordo/config.toml`（权限 600）。在 CI 里改用环境变量 `ORDO_TOKEN` 和 `ORDO_API_URL`。

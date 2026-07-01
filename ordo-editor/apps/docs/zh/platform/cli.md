# 命令行工具（`ordo`）

`ordo` CLI 把决策规则带进你的开发流程。一个项目就是一个文件夹——像源码一样编辑、本地校验（离线、亚秒），再同步到平台。它的设计目标是人和 AI 编码 agent 都能顺手用。

## 安装

```bash
# 免安装,直接跑
npx @ordo-engine/cli --help

# 或全局安装
npm i -g @ordo-engine/cli
ordo --help
```

安装时会下载对应平台的预编译静态二进制。也可从源码构建：`cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli`。

每个命令都支持 `--json` 输出机器可读结果。

## 磁盘上的决策项目

`ordo init` 生成一个项目——一棵和 Studio 模型对应的文件树：

```text
ordo.yaml              项目 + 链接配置
rulesets/<name>.json   一条规则(studio 格式)
facts.json             事实目录(外部输入)
concepts.json          概念目录(派生表达式)
tests/<name>.json      某条规则的测试用例
contracts/<name>.json  决策契约
AGENTS.md              给编码 agent 的说明
```

把这个文件夹放进 git——规则从此享受 PR、评审、CI,和代码一样。

## 本地闭环(离线)

```bash
ordo init my-rules && cd my-rules

ordo validate                 # 编译每个条件,结构化报错
ordo test                     # 跑规则的测试用例
ordo trace loan-approval --input '{"amount":5000}'   # 展示执行路径
ordo fmt                      # 规范化格式化规则文件
ordo lint                     # 图 + 风格检查
ordo new ruleset|fact|concept <name>
```

`validate`、`test`、`trace` 全部在本地内嵌引擎上运行——不联网、不需要服务器。概念的物化方式和平台一致,所以本地跑的结果和生产一致。

`ordo trace` 是调试利器:它打印一条输入在步骤间走过的确切路径——决策不符合预期时特别有用。

```text
$ ordo trace loan-approval --input '{"amount":5000}'
code:    APPROVED
output:  { "approved": true, "amount": 5000 }
path:    check_amount -> approve
```

## 与平台同步

平台是一个"远端",你从它 pull、往它 push/publish。

```bash
ordo login                                  # 认证(token 存在 ~/.ordo)
ordo link --org <org> --project <project>   # 把本地文件夹绑定到项目
ordo pull                                   # 拉取 rulesets + 目录 + 测试
# ...改文件...
ordo push                                   # 上传草稿(facts/concepts/tests 一并)
ordo publish loan-approval --env staging    # 发布到某环境
ordo deployments                            # 看部署状态
ordo diff                                   # 本地 vs 服务端草稿
```

`push` 是全量同步:rulesets、facts、concepts、每条规则的 tests、contracts(`--rulesets-only` 只推规则)。它用乐观锁——服务端有更新的改动时会提示你先 `ordo pull`。

### CI

因为本地命令离线且返回正确的退出码,可以直接进 CI:

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
```

### 配置与环境变量

认证和 API 地址存在 `~/.ordo/config.toml`(chmod 600)。CI 里改用 `ORDO_TOKEN` 和 `ORDO_API_URL`——它们覆盖配置文件。

## 让 AI agent 来驱动

`ordo mcp` 通过 Model Context Protocol 把这些工具暴露给编码 agent(Claude Code、Cursor),它就能替你写、测、发规则。见 [MCP](/zh/platform/mcp)。

## Shell 补全

```bash
ordo completions zsh > ~/.zfunc/_ordo    # bash | zsh | fish | powershell | elvish
```

## 命令一览

| 分组     | 命令                                                                        |
| -------- | --------------------------------------------------------------------------- |
| 脚手架   | `init`、`new`                                                               |
| 本地闭环 | `validate`、`test`、`trace`、`exec`、`eval`、`fmt`、`lint`                  |
| 平台     | `login`、`whoami`、`link`、`pull`、`push`、`publish`、`deployments`、`diff` |
| Agent    | `mcp`                                                                       |
| 其它     | `completions`                                                               |

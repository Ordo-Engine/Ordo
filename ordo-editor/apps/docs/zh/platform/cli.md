# 命令行工具（`ordo`）

`ordo` CLI 让你在开发流程里管理决策规则。一个项目就是一个文件夹：像源码一样编辑，在本地校验（离线，一秒以内），再同步到平台。人和 AI 编码 agent 都可以使用。

## 安装

```bash
# 免安装,直接跑
npx @ordo-engine/cli --help

# 或全局安装
npm i -g @ordo-engine/cli
ordo --help
```

安装时会下载对应平台的预编译静态二进制。也可以从源码构建：`cargo install --git https://github.com/Ordo-Engine/Ordo ordo-cli`。

每个命令都支持 `--json`，输出机器可读的结果。

## 项目的文件结构

`ordo init` 生成一个项目，文件树和 Studio 的模型一一对应：

```text
ordo.yaml              项目 + 链接配置
rulesets/<name>.json   一条规则(studio 格式)
facts.json             事实目录(外部输入)
concepts.json          概念目录(派生表达式)
tests/<name>.json      某条规则的测试用例
contracts/<name>.json  决策契约
AGENTS.md              给编码 agent 的说明
```

把这个文件夹放进 git，规则就能和代码一样走 PR、评审和 CI。

## 本地校验（离线）

```bash
ordo init my-rules && cd my-rules

ordo validate                 # 编译每个条件,结构化报错
ordo test                     # 跑规则的测试用例
ordo trace loan-approval --input '{"amount":5000}'   # 展示执行路径
ordo impact loan-approval     # 和上次提交比，哪些决策变了
ordo fmt                      # 规范化格式化规则文件
ordo lint                     # 图 + 风格检查
ordo new ruleset|fact|concept <name>
```

`validate`、`test`、`trace` 都在本地内嵌引擎上运行，不联网，不需要服务器。概念的物化方式和平台一致，所以本地结果和生产一致。

`ordo trace` 打印一条输入在各步骤间走过的路径，决策结果不符合预期时可以用它排查。

```text
$ ordo trace loan-approval --input '{"amount":5000}'
code:    APPROVED
output:  { "approved": true, "amount": 5000 }
path:    check_amount -> approve
```

### 改完规则，看哪些决策变了

`ordo test` 只能告诉你已有用例还过不过。`ordo impact` 把上一次提交（git `HEAD`）的版本和你改过的版本各跑一遍，列出结果变了的每一条输入。

输入有三类：测试用例、`--inputs` 传入的真实案例（JSONL，格式同 `ordo replay`），以及自动生成的边界探测。规则里每出现一处 `amount <= 10000` 这样的比较，或决策表里的一个格子，都会在阈值下方、正好、上方各试一次。所以就算没有测试覆盖到阈值附近，改动也会被发现。

```text
$ ordo impact loan-approval        # 把 10000 改成了 15000，ordo test 仍然通过
impact loan-approval  (HEAD → working tree)
  8 inputs: 2 tests, 0 captured, 6 boundary probes

  REJECTED → APPROVED  3 inputs

CHANGED probe:amount=10001  {"amount":10001}
    code: REJECTED → APPROVED
    output.approved: false → true
...
```

`--base <rev>` 换一个对比的提交，`--base-file` 直接和某个文件比。`--json` 输出给 agent 读，`--fail-on-change` 在有变化时返回非零，可以放进 CI。

## 与平台同步

平台相当于一个远端，你从它 pull，向它 push 或 publish。

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

`push` 是全量同步：rulesets、facts、concepts、每条规则的 tests 和 contracts（`--rulesets-only` 只推规则）。它使用乐观锁，服务端有更新的改动时会提示你先 `ordo pull`。

### CI

本地命令离线运行并返回正确的退出码，可以直接用在 CI 里：

```yaml
- run: npx @ordo-engine/cli validate
- run: npx @ordo-engine/cli test
```

### 配置与环境变量

认证信息和 API 地址存在 `~/.ordo/config.toml`（chmod 600）。CI 里改用 `ORDO_TOKEN` 和 `ORDO_API_URL`，它们会覆盖配置文件。

## 交给 AI agent

`ordo mcp` 通过 Model Context Protocol 把这些工具提供给编码 agent（Claude Code、Cursor），由 agent 编写、测试和发布规则。见 [MCP](/zh/platform/mcp)。

## Shell 补全

```bash
ordo completions zsh > ~/.zfunc/_ordo    # bash | zsh | fish | powershell | elvish
```

## 命令一览

| 分组     | 命令                                                                        |
| -------- | --------------------------------------------------------------------------- |
| 脚手架   | `init`、`new`                                                               |
| 本地校验 | `validate`、`test`、`trace`、`impact`、`exec`、`eval`、`fmt`、`lint`        |
| 平台     | `login`、`whoami`、`link`、`pull`、`push`、`publish`、`deployments`、`diff` |
| Agent    | `mcp`                                                                       |
| 其它     | `completions`                                                               |

# 快速上手

五分钟发出你的第一个决策——建项目、用模板写规则、拿样例试跑、发布,再从你的应用里调用它。无需自建引擎,平台会替你跑一个。

> 第一次接触 Ordo?先扫一眼[平台概览](./overview)建立心智模型(建模 → 编写 → 测试 → 发布 → 运行)。本页是动手路径。

有两条入口,任选其一——结果一样,而且随时可切换:[CLI](./cli) 能拉取你在 [Studio](./studio) 里搭的东西,Studio 也会显示你从 CLI 推上去的改动。

- **Studio(网页)**——浏览器里点选式搭建,最适合第一次上手。
- **CLI(本地)**——规则即文件,放进你的 git 仓库,由你或 AI 编码 agent 驱动。

## 路线 A —— Studio(网页)

### 1. 建项目

登录 Studio,先建一个**组织**,再在里面建一个**项目**。项目是承载事实、概念、规则集、环境以及它所运行引擎的基本单元。见[组织与项目](./organizations)。

### 2. 从模板开始

新建规则集 → 选 **Loan Approval**(或 Ecommerce Coupon)。你会得到一个能跑的决策图——一个对 `amount` 做判断的决策步 + 通过/拒绝两个终止步——而且[事实目录](./catalog)已经预填好它读取的输入。

### 3. 试跑

打开 trace 面板,粘贴一段样例输入,点 **Try run**:

```json
{ "amount": 5000, "is_vip": true }
```

你会看到命中的分支、完整路径、每步耗时,以及终止步的 `code` / `output`。这就是服务生产的同一个引擎——[Studio 编辑器](./studio)讲了三种视图和 trace 面板。

### 4. 发布

向某个环境发起一次发布(先用 **staging**)。测试和 diff 会自动跑;审批通过后,平台把规则投递到该环境的引擎。见[发布流程](./releases)。

### 5. 调用它

你的应用在运行时调用引擎——见[运行时接入](./integrate):

```bash
POST https://<engine>/api/v1/execute/loan-approval
Header: x-tenant-id: <项目id>
Body:   { "input": { "amount": 5000, "is_vip": true } }
```

```json
{ "code": "APPROVED", "output": { "approved": true }, "duration_us": 6 }
```

## 路线 B —— CLI(本地,git 原生)

上面这一切,都以文件形式放进你的仓库。无需安装——`npx` 会拉取预编译二进制。

### 1. 脚手架 + 本地闭环(离线)

```bash
npx @ordo-engine/cli init my-rules && cd my-rules

ordo validate     # 编译每个条件,结构化报错
ordo test         # 跑规则集的测试用例
ordo trace loan-approval --input '{"amount":5000,"is_vip":true}'
```

`validate` / `test` / `trace` 跑在内嵌引擎上——离线、亚秒,概念物化方式和生产一致。见 [CLI](./cli)。

### 2. 连上平台

```bash
ordo login
ordo link --org <org> --project <project>
ordo push                                # rulesets + facts + concepts + tests
ordo publish loan-approval --env staging
```

### 3. 让 AI agent 来驱动

```bash
claude mcp add ordo -- ordo mcp
```

现在你的编码 agent 原生拥有了 Ordo 的工具——它能在本地项目上读、写、校验、测试、trace 规则,并提议由你审批的发布。见 [MCP 服务](./mcp)。

### 4. 调用它

和路线 A 一样的运行时调用 → [运行时接入](./integrate)。

## 你刚刚搭出了什么

| 部件         | 是什么                                              |
| ------------ | --------------------------------------------------- |
| **项目**     | 承载事实、规则集、环境以及绑定的引擎                |
| **规则集**   | 你编写并测试过的决策图                              |
| **环境**     | 已发布版本运行的地方(staging → prod)                |
| **引擎调用** | `POST /api/v1/execute/<name>`,用你的项目作为 tenant |

## 下一步

- [事实目录](./catalog) · [决策契约](./contracts) —— 建模类型化的输入与 I/O
- [发布流程](./releases) —— 评审、灰度、回滚
- [测试管理](./testing) —— 用例、套件、CI
- [运行时接入](./integrate) —— REST、gRPC 与官方 SDK

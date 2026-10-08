# 快速上手

本页带你在五分钟内发出第一个决策：建项目、用模板写规则、用样例试跑、发布，再从应用里调用。不需要自建引擎，平台会运行一个。

> 第一次接触 Ordo？可以先看[平台概览](./overview)了解整体流程（建模 → 编写 → 测试 → 发布 → 运行）。本页是动手步骤。

有两条入口，结果相同，可以随时切换：[CLI](./cli) 能拉取你在 [Studio](./studio) 里搭建的内容，Studio 也会显示从 CLI 推送的改动。

- **Studio（网页）**：在浏览器里点选搭建，适合第一次上手。
- **CLI（本地）**：规则以文件形式放进 git 仓库，由你或 AI 编码 agent 操作。

## 路线 A：Studio（网页）

### 1. 建项目

登录 Studio，先建一个组织，再在组织里建一个项目。项目是承载事实、概念、规则集、环境以及所用引擎的基本单元。见[组织与项目](./organizations)。

### 2. 从模板开始

新建规则集，选择 **Loan Approval**（或 Ecommerce Coupon）。你会得到一个可运行的决策图：一个判断 `amount` 的决策步，加上通过、拒绝两个终止步。[事实目录](./catalog)里已经预填了它读取的输入。

### 3. 试跑

打开 trace 面板，粘贴一段样例输入，点击 **Try run**：

```json
{ "amount": 5000, "is_vip": true }
```

你会看到命中的分支、完整路径、每步耗时，以及终止步的 `code` / `output`。这里运行的引擎和生产环境是同一个。[Studio 编辑器](./studio)介绍了三种视图和 trace 面板。

### 4. 发布

向某个环境发起发布（先用 staging）。测试和 diff 会自动运行，审批通过后，平台把规则下发到该环境的引擎。见[发布流程](./releases)。

### 5. 调用

应用在运行时调用引擎，见[运行时接入](./integrate)：

```bash
POST https://<engine>/api/v1/execute/loan-approval
Header: x-tenant-id: <项目id>
Body:   { "input": { "amount": 5000, "is_vip": true } }
```

```json
{ "code": "APPROVED", "output": { "approved": true }, "duration_us": 6 }
```

## 路线 B：CLI（本地，基于 git）

同样的内容以文件形式放在你的仓库里。无需安装，`npx` 会拉取预编译二进制。

### 1. 生成项目并在本地验证（离线）

```bash
npx @ordo-engine/cli init my-rules && cd my-rules

ordo validate     # 编译每个条件,结构化报错
ordo test         # 跑规则集的测试用例
ordo trace loan-approval --input '{"amount":5000,"is_vip":true}'
```

`validate` / `test` / `trace` 在内嵌引擎上运行，离线，耗时在一秒以内，概念的物化方式和生产一致。见 [CLI](./cli)。

### 2. 连接平台

```bash
ordo login
ordo link --org <org> --project <project>
ordo push                                # rulesets + facts + concepts + tests
ordo publish loan-approval --env staging
```

### 3. 交给 AI agent

```bash
claude mcp add ordo -- ordo mcp
```

添加后，编码 agent 可以使用 Ordo 的工具：在本地项目上读、写、校验、测试、trace 规则，并提交发布请求，由你审批。见 [MCP 服务](./mcp)。

### 4. 调用

运行时调用和路线 A 相同，见[运行时接入](./integrate)。

## 你搭建了什么

| 部件         | 是什么                                              |
| ------------ | --------------------------------------------------- |
| **项目**     | 承载事实、规则集、环境以及绑定的引擎                |
| **规则集**   | 你编写并测试过的决策图                              |
| **环境**     | 已发布版本运行的地方（staging → prod）               |
| **引擎调用** | `POST /api/v1/execute/<name>`，以项目作为 tenant |

## 下一步

- [事实目录](./catalog) · [决策契约](./contracts)：为输入和 I/O 建模类型
- [发布流程](./releases)：评审、灰度、回滚
- [测试管理](./testing)：用例、套件、CI
- [运行时接入](./integrate)：REST、gRPC 与官方 SDK

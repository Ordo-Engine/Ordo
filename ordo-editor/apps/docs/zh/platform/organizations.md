# 组织与项目

Ordo Platform 的所有资源都挂在「组织 → 项目」两层结构下。组织是计费与治理边界，项目是规则、契约、测试套件的聚合点。

## 组织（Organization）

- 用户注册后可创建任意数量的组织。
- 组织拥有独立的成员列表、角色定义、通知与项目。
- 支持 **子组织**（sub-organization），用于按业务线/部门进一步隔离。
- 组织级 API：`/api/v1/orgs`、`/api/v1/orgs/:id`、`/api/v1/orgs/:id/sub-orgs`。

## 成员与角色（RBAC）

平台内置一套 RBAC，支持自定义角色：

| 内置角色 | 权限范围                         |
| -------- | -------------------------------- |
| `owner`  | 组织全部权限，包括成员管理与计费 |
| `admin`  | 项目级管理、审批、发布           |
| `editor` | 草稿创作与发起发布，无审批权     |
| `viewer` | 只读                             |

自定义角色：`POST /api/v1/orgs/:oid/roles`，可精细到 `release.approve`、`ruleset.publish` 等动作粒度。

成员管理：

- `POST /api/v1/orgs/:id/members` —— 邀请成员
- `PUT /api/v1/orgs/:oid/members/:uid/roles` —— 调整角色
- 子组织成员：`/api/v1/orgs/:parent_id/sub-orgs/:sub_id/members`

## 项目（Project）

项目是真正持有规则集与契约的容器。

- 创建：`POST /api/v1/orgs/:oid/projects`
- 模板创建：`POST /api/v1/orgs/:oid/projects/from-template` —— 从内置模板（电商优惠券、贷款审批等）一键实例化
- 项目结构：
  - **环境**（environments）：默认 `dev` / `staging` / `prod`，可自定义
  - **事实目录**（facts）：项目级类型化字段定义
  - **概念**（concepts）：跨规则集复用的复合类型
  - **契约**（contracts）：输入/输出 Schema
  - **规则集**（rulesets）：业务决策逻辑
  - **子规则资产**（sub-rules）：跨规则集复用的逻辑片段
  - **测试套件**（tests）
  - **发布策略**（release policies）
  - **绑定的服务器**（server）：执行集群

## 项目级 API 速查

| 资源     | 端点                                           |
| -------- | ---------------------------------------------- |
| 项目     | `/api/v1/orgs/:oid/projects/:pid`              |
| 环境     | `/api/v1/orgs/:oid/projects/:pid/environments` |
| 事实     | `/api/v1/projects/:pid/facts`                  |
| 概念     | `/api/v1/projects/:pid/concepts`               |
| 契约     | `/api/v1/projects/:pid/contracts`              |
| 规则集   | `/api/v1/orgs/:oid/projects/:pid/rulesets`     |
| 子规则   | `/api/v1/orgs/:oid/projects/:pid/sub-rules`    |
| 测试套件 | `/api/v1/projects/:pid/rulesets/:name/tests`   |
| 发布请求 | `/api/v1/orgs/:oid/projects/:pid/releases`     |
| 引擎代理 | `/api/v1/engine/:project_id/*path`             |

## 通知

每个组织都有自己的通知队列，覆盖：

- 待审批的发布请求
- 发布失败 / 灰度异常
- 测试套件失败
- 成员邀请与权限变更

API：

- `GET /api/v1/orgs/:oid/notifications`
- `GET /api/v1/orgs/:oid/notifications/count`
- `POST /api/v1/orgs/:oid/notifications/:nid/read`
- `POST /api/v1/orgs/:oid/notifications/read-all`

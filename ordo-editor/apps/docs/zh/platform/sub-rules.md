# 子规则资产

子规则（Sub-Rule）让你把跨规则集复用的逻辑片段抽取成项目级或组织级资产，多个规则集通过 `SubRule` 步骤引用同一份资产。

## 应用场景

- **KYC 审核**：不同业务线（贷款、信用卡、保险）都需要相同的身份核验逻辑。
- **风险评分**：客户风险分数算法在多个决策中复用。
- **黑名单检查**：所有面向用户的规则集开头都要走一遍。

## 数据模型

```jsonc
// POST /api/v1/orgs/:oid/projects/:pid/sub-rules
{
  "name": "kyc-check",
  "version": "1.2.0",
  "graph": {
    "startStepId": "verify",
    "steps": [
      { "id": "verify", "type": "decision", "branches": [...] },
      { "id": "pass",   "type": "terminal", "code": "OK" },
      { "id": "fail",   "type": "terminal", "code": "REJECT" }
    ]
  },
  "bindings": [
    { "name": "id_number", "type": "string", "required": true }
  ],
  "outputs": [
    { "name": "score", "type": "number" }
  ]
}
```

- `bindings`：调用方必须传入的参数。
- `outputs`：子规则结束后回写到父级上下文的字段。

## 在规则集中引用

Studio 中放置一个 `SubRule` 节点，选择 ref name 与版本，配置 binding 表达式与 output 映射：

```jsonc
{
  "id": "step_kyc",
  "type": "sub_rule",
  "refName": "kyc-check",
  "bindings": [{ "name": "id_number", "value": { "type": "variable", "path": "$.user.idn" } }],
  "outputs": [{ "name": "score", "to": "kyc_score" }],
  "nextStepId": "step_decide"
}
```

## 发布时的内联快照

发布规则集时，平台会**深度内联**所有引用的子规则当前版本（BFS 解析），生成一个无外部依赖的扁平 RuleSet 再下发到 ordo-server：

- 子规则后续被修改、删除，已发布的规则集行为完全不变。
- 引擎执行时不再需要回头查询子规则，零额外开销。
- 默认调用深度上限 10（避免循环递归），平台同时做 DFS 环检测。

## 版本与 diff

每次更新子规则会形成新版本快照：

| 操作   | 端点                                                      |
| ------ | --------------------------------------------------------- |
| 列出   | `GET  /api/v1/orgs/:oid/projects/:pid/sub-rules`          |
| 取/改  | `GET/PUT /api/v1/orgs/:oid/projects/:pid/sub-rules/:name` |
| 组织级 | `/api/v1/orgs/:oid/sub-rules`（跨项目共享）               |

修改子规则后，平台会列出**所有引用它的规则集**——这些规则集需要重新发布才能拿到新版子规则的逻辑。

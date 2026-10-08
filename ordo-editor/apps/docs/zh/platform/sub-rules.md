# 子规则资产

**子规则**（Sub-Rule）是项目级或组织级的可复用逻辑片段。多个规则集通过 `SubRule` 步骤引用同一份资产。

## 应用场景

- **KYC 审核**：不同业务线（贷款、信用卡、保险）都需要相同的身份核验逻辑。
- **风险评分**：客户风险分数算法在多个决策中复用。
- **黑名单检查**：所有面向用户的规则集开头都要执行一次。

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

在 Studio 中放置一个 `SubRule` 节点，选择 ref name 与版本，配置 binding 表达式与 output 映射：

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

发布规则集时，平台会把引用的所有子规则的当前版本逐层内联（BFS 解析），生成一个没有外部依赖的扁平 RuleSet，再下发到 ordo-server：

- 子规则之后被修改或删除，已发布规则集的行为不受影响。
- 引擎执行时不需要再查询子规则，没有额外开销。
- 默认调用深度上限为 10（避免循环递归），平台同时用 DFS 做环检测。

## 版本与 diff

每次更新子规则都会生成新的版本快照：

| 操作   | 端点                                                      |
| ------ | --------------------------------------------------------- |
| 列出   | `GET  /api/v1/orgs/:oid/projects/:pid/sub-rules`          |
| 取/改  | `GET/PUT /api/v1/orgs/:oid/projects/:pid/sub-rules/:name` |
| 组织级 | `/api/v1/orgs/:oid/sub-rules`（跨项目共享）               |

修改子规则后，平台会列出所有引用它的规则集。这些规则集需要重新发布，才能使用新版子规则的逻辑。

# HTTP API

基础地址：`http://localhost:8080`。请求体和响应体都是 JSON。

开启 `--multi-tenancy-enabled` 时，每个请求都要带 `X-Tenant-ID`。开启 `--api-token` 时，带 `Authorization: Bearer <token>` 或 `X-API-Token: <token>`。健康检查和指标两者都不需要。

## 接口

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| `POST` | `/api/v1/execute/:name` | 执行规则集 |
| `POST` | `/api/v1/execute/:name/batch` | 对多条输入执行规则集 |
| `POST` | `/api/v1/execute-pipeline` | 依次执行几个规则集 |
| `POST` | `/api/v1/eval` | 计算一个表达式 |
| `GET` | `/api/v1/rulesets` | 列出规则集 |
| `POST` | `/api/v1/rulesets` | 创建或更新规则集 |
| `GET`、`DELETE` | `/api/v1/rulesets/:name` | 获取或删除规则集 |
| `GET` | `/api/v1/rulesets/:name/versions` | 列出保存的版本 |
| `GET` | `/api/v1/rulesets/:name/versions/:seq` | 获取某个保存的版本 |
| `POST` | `/api/v1/rulesets/:name/rollback` | 回滚到某个保存的版本 |
| `POST` | `/api/v1/rulesets/:name/test` | 对规则集跑测试用例 |
| `POST` | `/api/v1/rulesets/:name/filter` | 把规则集转成数据库过滤条件 |
| `POST` | `/api/v1/admin/reload` | 从 `--rules-dir` 重新加载所有规则 |
| `GET`、`PUT` | `/api/v1/config/audit-sample-rate` | 查看或修改审计采样率 |
| `GET`、`POST` | `/api/v1/tenants` | 列出或创建租户 |
| `GET`、`PUT`、`DELETE` | `/api/v1/tenants/:id` | 管理单个租户 |
| `GET`、`POST` | `/api/v1/webhooks` | 列出或添加 webhook |
| `GET`、`PUT`、`DELETE` | `/api/v1/webhooks/:id` | 管理单个 webhook |
| `POST` | `/systemone/v1/systemone` | 决策模型网关，见[决策模型网关](/zh/platform/decision-gateway) |
| `GET` | `/health`、`/healthz/live`、`/healthz/ready` | 健康检查 |
| `GET` | `/metrics` | Prometheus 指标，见[指标](/zh/reference/metrics) |

开启 `--debug-mode` 后会多出 `/api/v1/debug/` 下的接口。生产环境不要开。

## 执行规则集

```http
POST /api/v1/execute/discount
```

```json
{ "input": { "user": { "tier": "gold" }, "order": { "amount": 1200 } }, "trace": true }
```

```json
{
  "code": "OK",
  "message": "",
  "output": { "rate": 0.15, "pay": 1020.0 },
  "duration_us": 103,
  "trace": {
    "path": "pick_rate -> done",
    "steps": [
      { "id": "pick_rate", "name": "Pick discount rate", "duration_us": 53 },
      { "id": "done", "name": "Done", "duration_us": 0 }
    ]
  }
}
```

`trace` 可选，默认关闭。

## 批量执行

```http
POST /api/v1/execute/discount/batch
```

```json
{ "inputs": [ { "user": { "tier": "gold" }, "order": { "amount": 1200 } }, { "...": "..." } ] }
```

```json
{
  "results": [ { "code": "OK", "message": "", "output": { "rate": 0.15, "pay": 1020.0 }, "duration_us": 91 } ],
  "summary": { "total": 1, "success": 1, "failed": 0, "total_duration_us": 91 }
}
```

## 依次执行几个规则集

```http
POST /api/v1/execute-pipeline
```

```json
{ "rulesets": ["discount", "shipping"], "input": { "...": "..." } }
```

响应的 `stages` 里每个规则集一项，另有最终的 `output`。

## 管理规则集

`POST /api/v1/rulesets`，请求体是规则集。新建返回 `201` 和 `{"name": "discount", "status": "created"}`，替换返回 `200` 和 `"status": "updated"`。编译不通过的规则集返回 `400`。开启签名校验时见[签名规则](/zh/guide/deployment#签名规则)。

`GET /api/v1/rulesets` 返回 `[{ "name", "version", "description", "step_count" }]`。`DELETE` 返回 `204`。

## 版本

需要 `--rules-dir`。

```http
GET /api/v1/rulesets/discount/versions
```

```json
{ "name": "discount", "current_version": "1.1.0",
  "versions": [ { "seq": 1, "version": "1.0.0", "created_at": "2026-10-14T06:23:51Z" } ] }
```

```http
POST /api/v1/rulesets/discount/rollback
```

```json
{ "seq": 1 }
```

```json
{ "status": "rolled_back", "name": "discount", "from_version": "1.1.0", "to_version": "1.0.0" }
```

## 计算表达式

```http
POST /api/v1/eval
```

```json
{ "expression": "user.age >= 18", "context": { "user": { "age": 25 } } }
```

```json
{ "result": true, "parsed": "Binary { op: Ge, ... }" }
```

## 数据库过滤条件

像"这个用户能看哪些文档"这样的问题，对每一行都跑一遍规则很慢。过滤接口先代入已知的信息（用户），再把规则剩下的部分转成 `WHERE` 条件。

```http
POST /api/v1/rulesets/doc_access/filter
```

```json
{
  "known_input": { "user": { "role": "member", "id": "alice" } },
  "target_results": ["ALLOW"],
  "format": "sql",
  "field_mapping": { "doc.owner_id": "owner_id", "doc.visibility": "visibility", "doc.status": "status" }
}
```

```json
{
  "format": "sql",
  "filter": "(owner_id = 'alice') OR ((visibility = 'public' AND status = 'published'))",
  "always_matches": false,
  "never_matches": false,
  "unknown_fields": ["doc.owner_id", "doc.status", "doc.visibility"]
}
```

| 请求字段         | 含义 |
| ---------------- | ---- |
| `known_input`    | 查询时已知的字段 |
| `target_results` | 算作匹配的结果码 |
| `format`         | `sql`（默认）、`json` 或 `mongo` |
| `field_mapping`  | 规则字段到列名的映射。没映射的字段用路径把 `.` 换成 `_` |
| `max_paths`      | 最多收集多少条路径，默认 `100`，`0` 表示不限 |

`always_matches` 为真时（比如管理员），不用加 `WHERE`。`never_matches` 为真时，`filter` 是 `null`，结果为空。如果达到了 `max_paths`，`truncated` 为真，同时 `always_matches` 也会设为真，保证不会漏掉数据。

## 错误

```json
{ "code": "NOT_FOUND", "message": "RuleSet 'nope' not found" }
```

| 状态码 | `code` | 什么时候 |
| --- | --- | --- |
| `400` | `BAD_REQUEST`、`INVALID_INPUT` | 请求体不对、规则集编译不过、输入没通过输入校验 |
| `401` | `UNAUTHORIZED` | 缺少或错误的 API token |
| `403` | `FORBIDDEN` | 签名校验失败 |
| `404` | `NOT_FOUND` | 规则集或版本不存在 |
| `408` | | 超过 `--request-timeout-secs` |
| `409` | `CONFLICT`，或 `error` 为 `read_only` | 写冲突，或写请求发到了读节点 |
| `500` | `INTERNAL_ERROR` | 执行失败，比如 `field_missing: strict` 时缺字段 |

# HTTP API

Base URL: `http://localhost:8080`. Request and response bodies are JSON.

With `--multi-tenancy-enabled`, send `X-Tenant-ID` on every request. With `--api-token`, send `Authorization: Bearer <token>` or `X-API-Token: <token>`. Health and metrics need neither.

## Endpoints

| Method | Path | What it does |
| --- | --- | --- |
| `POST` | `/api/v1/execute/:name` | Run a ruleset |
| `POST` | `/api/v1/execute/:name/batch` | Run a ruleset on many inputs |
| `POST` | `/api/v1/execute-pipeline` | Run several rulesets in a row |
| `POST` | `/api/v1/eval` | Evaluate one expression |
| `GET` | `/api/v1/rulesets` | List rulesets |
| `POST` | `/api/v1/rulesets` | Create or update a ruleset |
| `GET`, `DELETE` | `/api/v1/rulesets/:name` | Get or delete a ruleset |
| `GET` | `/api/v1/rulesets/:name/versions` | List saved versions |
| `GET` | `/api/v1/rulesets/:name/versions/:seq` | Get one saved version |
| `POST` | `/api/v1/rulesets/:name/rollback` | Roll back to a saved version |
| `POST` | `/api/v1/rulesets/:name/test` | Run test cases against a ruleset |
| `POST` | `/api/v1/rulesets/:name/filter` | Turn a ruleset into a database filter |
| `POST` | `/api/v1/admin/reload` | Reload every rule from `--rules-dir` |
| `GET` | `/api/v1/data` | List external data names |
| `GET`, `PUT`, `DELETE` | `/api/v1/data/:name` | Read, set or delete external data, read in rules as `$data.<name>` |
| `GET`, `PUT` | `/api/v1/config/audit-sample-rate` | Read or change the audit sample rate |
| `GET`, `POST` | `/api/v1/tenants` | List or create tenants |
| `GET`, `PUT`, `DELETE` | `/api/v1/tenants/:id` | Manage one tenant |
| `GET`, `POST` | `/api/v1/webhooks` | List or add webhooks |
| `GET`, `PUT`, `DELETE` | `/api/v1/webhooks/:id` | Manage one webhook |
| `POST` | `/systemone/v1/systemone` | Decision model gateway. See [Decision Model Gateway](/en/platform/decision-gateway) |
| `GET` | `/health`, `/healthz/live`, `/healthz/ready` | Health checks |
| `GET` | `/metrics` | Prometheus metrics. See [Metrics](/en/reference/metrics) |

With `--debug-mode`, extra routes under `/api/v1/debug/` are added. Do not turn it on in production.

## Run a ruleset

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

`trace` is optional and off by default.

## Run many inputs

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

## Run several rulesets in a row

```http
POST /api/v1/execute-pipeline
```

```json
{ "rulesets": ["discount", "shipping"], "input": { "...": "..." } }
```

The response has one entry per ruleset in `stages`, plus the final `output`. Each stage gets the input plus the outputs of the stages before it.

## External data

`PUT /api/v1/data/limits` with any JSON value as the body stores it (`200`). Every ruleset of the tenant can then read it as `$data.limits...`. `GET /api/v1/data/limits` returns it, `DELETE` removes it, `GET /api/v1/data` lists the names. See [Running the Server](/en/guide/server#external-data).

## Manage rulesets

`POST /api/v1/rulesets` with a ruleset as the body creates it (`201`, `{"name": "discount", "status": "created"}`) or replaces it (`200`, `"status": "updated"`). A ruleset that does not compile is rejected with `400`. With signature checks on, see [Signed rules](/en/guide/deployment#signed-rules).

`GET /api/v1/rulesets` returns `[{ "name", "version", "description", "step_count" }]`. `DELETE` returns `204`.

## Versions

Needs `--rules-dir`.

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

## Evaluate an expression

```http
POST /api/v1/eval
```

```json
{ "expression": "user.age >= 18", "context": { "user": { "age": 25 } } }
```

```json
{ "result": true, "parsed": "Binary { op: Ge, ... }" }
```

## Database filter

For questions like "which documents may this user see", running the rule on every row is slow. The filter endpoint fills in what you already know (the user), and turns the rest of the rule into a `WHERE` clause.

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

| Request field    | Meaning |
| ---------------- | ------- |
| `known_input`    | Fields known at query time |
| `target_results` | Result codes that count as a match |
| `format`         | `sql` (default), `json` or `mongo` |
| `field_mapping`  | Rule field to column name. Unmapped fields become the path with `.` replaced by `_` |
| `max_paths`      | Stop after this many paths, default `100`, `0` for no limit |

When `always_matches` is true (an admin, say), skip the `WHERE`. When `never_matches` is true, `filter` is `null` and the answer is empty. If `max_paths` was hit, `truncated` is true and `always_matches` is set too, so you never miss rows.

## Errors

```json
{ "code": "NOT_FOUND", "message": "RuleSet 'nope' not found" }
```

| Status | `code` | When |
| --- | --- | --- |
| `400` | `BAD_REQUEST`, `INVALID_INPUT` | Bad body, a ruleset that does not compile, input that fails the input schema |
| `401` | `UNAUTHORIZED` | Missing or wrong API token |
| `403` | `FORBIDDEN` | Signature check failed |
| `404` | `NOT_FOUND` | No such ruleset or version |
| `408` | | Took longer than `--request-timeout-secs` |
| `409` | `CONFLICT`, or `read_only` in `error` | A conflicting write, or a write sent to a reader instance |
| `500` | `INTERNAL_ERROR` | Execution failed, for example a missing field with `field_missing: strict` |

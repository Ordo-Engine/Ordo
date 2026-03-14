# 数据过滤 API

直接从规则集生成数据库过滤表达式——将规则逻辑下推到查询层，无需扫描整张表再逐行求值。

## 问题背景

典型的访问控制查询模式：

```
DB:  SELECT * FROM documents          ← 全表扫描
App: for each row → run ruleset      ← O(n) 次规则执行
App: 丢弃不匹配的行
```

使用数据过滤 API 后：

```
App: POST /rulesets/doc_access/filter  ← 单次调用
     → "owner_id = 'alice' OR visibility = 'public'"
DB:  SELECT * FROM documents
     WHERE owner_id = 'alice' OR visibility = 'public'   ← 走索引
```

## 接口端点

```http
POST /api/v1/rulesets/:name/filter
Content-Type: application/json
```

## 请求

```json
{
  "known_input": {
    "user": { "role": "member", "id": "alice", "subscription": "free" }
  },
  "target_results": ["ALLOW"],
  "format": "sql",
  "field_mapping": {
    "doc.owner_id": "owner_id",
    "doc.visibility": "visibility",
    "doc.status": "status"
  },
  "max_paths": 100
}
```

| 字段             | 类型                             | 必填 | 说明                                                                                                |
| ---------------- | -------------------------------- | ---- | --------------------------------------------------------------------------------------------------- |
| `known_input`    | object                           | ✅   | 查询时已知的字段（如当前用户会话）。支持嵌套路径：`{"user": {"id": "alice"}}` 通过 `user.id` 访问。 |
| `target_results` | string[]                         | ✅   | 代表"匹配"的结果码。指向其他终端的路径将被忽略。                                                    |
| `format`         | `"sql"` \| `"json"` \| `"mongo"` | —    | 输出格式。默认：`"sql"`。                                                                           |
| `field_mapping`  | object                           | —    | 将规则字段路径映射到数据库列名。未映射的字段默认将 `.` 替换为 `_`。                                 |
| `max_paths`      | number                           | —    | 收集的最大路径数，超出后停止。默认：`100`。`0` 表示不限制。                                         |

## 响应

```json
{
  "format": "sql",
  "filter": "(owner_id = 'alice') OR ((visibility = 'public' AND status = 'published'))",
  "always_matches": false,
  "never_matches": false,
  "unknown_fields": ["doc.owner_id", "doc.status", "doc.visibility"]
}
```

| 字段             | 类型                     | 说明                                                                                                            |
| ---------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------- |
| `filter`         | string \| object \| null | 生成的过滤条件。SQL 格式为字符串，JSON/Mongo 格式为对象，`never_matches` 为 true 时返回 `null`。                |
| `always_matches` | bool                     | 所有可能的输入都匹配，可跳过 WHERE 子句（如管理员用户）。                                                       |
| `never_matches`  | bool                     | 没有任何输入能匹配，可直接返回空结果。                                                                          |
| `truncated`      | bool                     | `max_paths` 限制在完整图遍历前被触发。此时 `always_matches` 也为 `true` 以避免漏行。请增大 `max_paths` 后重试。 |
| `unknown_fields` | string[]                 | 未被解析的规则字段——它们将作为列名出现在过滤条件中。                                                            |

## 工作原理

### 偏求值（Partial Evaluation）

给定 `known_input`，规则图中的每个字段引用都会被替换：

- `user.role == "admin"` 其中 `user.role = "viewer"` → `false` → 分支被消除
- `doc.owner_id == user.id` 其中 `user.id = "alice"` → `doc.owner_id == "alice"` → 保留为过滤条件

替换后优化器会进行常量折叠，因此像 `user.subscription == "premium" && doc.tier in ["free", "standard"]` 这样的复合表达式，在 `subscription` 已知的情况下会被正确折叠。

### 图遍历

规则图从入口步骤开始深度优先遍历：

- **Decision 步骤**：对每个分支条件进行偏求值
  - 恒为 false → 跳过该分支；其否定累积到默认路径
  - 恒为 true → 立即走该分支；后续分支为死代码
  - 未知 → 保留该分支及其条件；否定流向后续分支
- **Action 步骤**：透明透传（变量变更副作用不被追踪；下游字段被视为未知 DB 列，生成的是超集过滤条件）
- **Terminal 步骤**：若 `result.code` 在 `target_results` 中，累积的条件构成一条路径

同一路径内的条件用 AND 连接，多条路径之间用 OR 连接。

## 示例

### 基于角色的文档访问控制

给定如下规则图：

```
check_role
├── user.role == "admin"       → approved  (ALLOW)
├── user.role == "moderator"   → check_status
│   └── doc.status in ["published","review"] → approved  (ALLOW)
│   └── default → denied  (DENY)
├── user.role == "member"      → check_ownership
│   ├── doc.owner_id == user.id → approved  (ALLOW)
│   ├── doc.visibility == "public" && doc.status == "published" → approved  (ALLOW)
│   └── user.subscription == "premium" && doc.tier in ["free","standard"] → approved  (ALLOW)
│   └── default → denied  (DENY)
└── default → denied  (DENY)
```

**管理员 — `always_matches: true`，无需 WHERE 子句：**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets/doc_access/filter \
  -d '{ "known_input": { "user": { "role": "admin" } }, "target_results": ["ALLOW"] }'
```

```json
{ "filter": "TRUE", "always_matches": true }
```

**审核员 — 仅已发布/审核中的文档：**

```json
{
  "filter": "(status = 'published' OR status = 'review')"
}
```

**免费会员 alice — 仅自己的或公开文档：**

`subscription = "free"` 会将 `user.subscription == "premium"` 折叠为 false，消除高级会员路径。

```json
{
  "filter": "(owner_id = 'alice') OR ((visibility = 'public' AND status = 'published'))"
}
```

**高级会员 bob — 三条路径：**

```json
{
  "filter": "(owner_id = 'bob') OR ((visibility = 'public' AND status = 'published')) OR (tier IN ('free', 'standard'))"
}
```

**未知角色（访客）— `never_matches: true`：**

```json
{ "filter": null, "never_matches": true }
```

### MongoDB `$match` 格式

使用 `"format": "mongo"` 可获得 MongoDB 聚合管道的 `$match` 阶段，可直接传给 `db.collection.aggregate([{ $match: filter }])`。

**免费会员 alice：**

```json
{
  "filter": {
    "$or": [
      { "owner_id": "alice" },
      { "$and": [{ "visibility": "public" }, { "status": "published" }] }
    ]
  }
}
```

**支持的运算符：**

| 表达式                    | `$match` 输出                               |
| ------------------------- | ------------------------------------------- |
| `field == "x"`            | `{ col: "x" }`                              |
| `field == null`           | `{ col: null }`                             |
| `field != "x"`            | `{ col: { "$ne": "x" } }`                   |
| `field > n`               | `{ col: { "$gt": n } }`                     |
| `field in ["a","b"]`      | `{ col: { "$in": ["a","b"] } }`             |
| `field not in ["a","b"]`  | `{ col: { "$nin": ["a","b"] } }`            |
| `contains(field, "x")`    | `{ col: { "$regex": "x" } }`                |
| `starts_with(field, "x")` | `{ col: { "$regex": "^x" } }`               |
| `ends_with(field, "x")`   | `{ col: { "$regex": "x$" } }`               |
| `is_null(field)`          | `{ col: null }`                             |
| `!is_null(field)`         | `{ col: { "$ne": null, "$exists": true } }` |
| `a && b`                  | `{ "$and": [a, b] }`                        |
| `a \|\| b`                | `{ "$or": [a, b] }`                         |
| 多条路径                  | `{ "$or": [...] }`                          |
| 恒为匹配                  | `{}` (空文档，无过滤)                       |
| 恒不匹配                  | `{ "$expr": false }`                        |

字符串字面量中的正则元字符会自动转义。

### JSON 谓词格式

使用 `"format": "json"` 可获得结构化谓词树，ORM 和前端客户端可直接使用：

```json
{
  "type": "or",
  "conditions": [
    { "type": "eq", "field": "owner_id", "value": "bob" },
    {
      "type": "and",
      "conditions": [
        { "type": "eq", "field": "visibility", "value": "public" },
        { "type": "eq", "field": "status", "value": "published" }
      ]
    },
    { "type": "in", "field": "tier", "values": ["free", "standard"] }
  ]
}
```

**支持的节点类型：**

| 类型                          | 关键字段            | 含义                |
| ----------------------------- | ------------------- | ------------------- |
| `eq` `ne` `lt` `le` `gt` `ge` | `field`, `value`    | 比较                |
| `and` `or`                    | `conditions[]`      | 逻辑运算            |
| `not`                         | `condition`         | 取反                |
| `in` `not_in`                 | `field`, `values[]` | 集合成员            |
| `contains`                    | `field`, `value`    | 包含（子串 / 数组） |
| `is_null` `not_null`          | `field`             | NULL 检查           |
| `starts_with` `ends_with`     | `field`, `value`    | 前缀 / 后缀         |
| `always`                      | —                   | 无需过滤            |
| `never`                       | —                   | 空结果              |

## SQL 生成参考

| 表达式                    | SQL                         |
| ------------------------- | --------------------------- |
| `field == "x"`            | `col = 'x'`                 |
| `field == null`           | `col IS NULL`               |
| `field != "x"`            | `col != 'x'`                |
| `field != null`           | `col IS NOT NULL`           |
| `field in ["a","b"]`      | `col IN ('a', 'b')`         |
| `field not in ["a","b"]`  | `col NOT IN ('a', 'b')`     |
| `contains(field, "x")`    | `col LIKE '%x%' ESCAPE '!'` |
| `starts_with(field, "x")` | `col LIKE 'x%' ESCAPE '!'`  |
| `ends_with(field, "x")`   | `col LIKE '%x' ESCAPE '!'`  |
| `is_null(field)`          | `col IS NULL`               |
| `!is_null(field)`         | `col IS NOT NULL`           |
| `a && b`                  | `(a AND b)`                 |
| `a \|\| b`                | `(a OR b)`                  |
| 多条路径                  | `(...) OR (...)`            |

字符串字面量使用单引号转义（`'` → `''`）。LIKE 模式字面量还会额外转义 `!` → `!!`、`%` → `!%`、`_` → `!_`，保证值中的通配符被字面对待。空值比较使用 `IS NULL` / `IS NOT NULL` 以匹配 SQL 三值逻辑。算术运算符和不支持的函数将返回 `500` 错误。

## 错误

| 状态码 | 说明                                                |
| ------ | --------------------------------------------------- |
| 400    | `target_results` 为空                               |
| 404    | 规则集不存在                                        |
| 500    | 过滤器编译失败（如 SQL 模式中使用了不支持的运算符） |

## 已知限制

- **Action 步骤的变量变更**：`SetVariable` 的副作用不被追踪。引用了被修改变量的下游条件将被视为未知列——过滤器可能是一个超集，应用层执行最终过滤。
- **深度限制**：最大遍历 50 个步骤（硬限制，防止有环图的无限循环）。

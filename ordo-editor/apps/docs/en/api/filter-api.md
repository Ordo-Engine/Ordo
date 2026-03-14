# Data Filter API

Generate a database filter expression directly from a ruleset — push rule logic into your query layer instead of fetching all rows and evaluating each one.

## The Problem

A typical access-control query pattern:

```
DB:  SELECT * FROM documents          ← full table scan
App: for each row → run ruleset      ← O(n) rule executions
App: discard non-matching rows
```

With the Data Filter API:

```
App: POST /rulesets/doc_access/filter  ← one call
     → "owner_id = 'alice' OR visibility = 'public'"
DB:  SELECT * FROM documents
     WHERE owner_id = 'alice' OR visibility = 'public'   ← index-friendly
```

## Endpoint

```http
POST /api/v1/rulesets/:name/filter
Content-Type: application/json
```

## Request

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

| Field            | Type                             | Required | Description                                                                                                                                  |
| ---------------- | -------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `known_input`    | object                           | ✅       | Fields already known at query time (e.g. current user session). Supports nested paths: `{"user": {"id": "alice"}}` is accessed as `user.id`. |
| `target_results` | string[]                         | ✅       | Result codes that mean "match". Paths leading to any other terminal are ignored.                                                             |
| `format`         | `"sql"` \| `"json"` \| `"mongo"` | —        | Output format. Default: `"sql"`.                                                                                                             |
| `field_mapping`  | object                           | —        | Maps rule field paths to database column names. Unmapped fields default to the path with `.` replaced by `_`.                                |
| `max_paths`      | number                           | —        | Maximum paths to collect before stopping. Default: `100`. `0` means unlimited.                                                               |

## Response

```json
{
  "format": "sql",
  "filter": "(owner_id = 'alice') OR ((visibility = 'public' AND status = 'published'))",
  "always_matches": false,
  "never_matches": false,
  "unknown_fields": ["doc.owner_id", "doc.status", "doc.visibility"]
}
```

| Field            | Type                     | Description                                                                                                                                                     |
| ---------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `filter`         | string \| object \| null | The generated filter. String for SQL, object for JSON/Mongo, `null` when `never_matches` is true.                                                               |
| `always_matches` | bool                     | Every possible input matches. Skip the WHERE clause entirely (e.g. admin users).                                                                                |
| `never_matches`  | bool                     | No input can ever match. Return an empty result immediately.                                                                                                    |
| `truncated`      | bool                     | The `max_paths` limit was reached before the full graph was explored. `always_matches` is also `true` to avoid false negatives. Increase `max_paths` and retry. |
| `unknown_fields` | string[]                 | Rule fields that remained unresolved — they appear as columns in the filter.                                                                                    |

## How It Works

### Partial Evaluation

Given `known_input`, every field reference in the rule graph is substituted:

- `user.role == "admin"` where `user.role = "viewer"` → `false` → branch eliminated
- `doc.owner_id == user.id` where `user.id = "alice"` → `doc.owner_id == "alice"` → kept as filter condition

The constant-folding optimizer runs after substitution, so composite expressions like `user.subscription == "premium" && doc.tier in ["free", "standard"]` are correctly folded when `subscription` is known.

### Graph Traversal

The rule graph is traversed depth-first from the entry step:

- **Decision step**: each branch condition is partially evaluated
  - Always-false → branch skipped; its negation accumulates toward the default path
  - Always-true → branch taken immediately; subsequent branches are dead code
  - Unknown → branch included with its condition; negation flows to later branches
- **Action step**: transparent pass-through (variable mutations are not tracked; downstream fields are treated as unknown DB columns, producing a superset filter)
- **Terminal step**: if `result.code` is in `target_results`, the accumulated conditions become a path

Conditions within a path are ANDed; multiple paths are ORed.

## Examples

### Role-Based Document Access

Given this rule graph:

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

**Admin — `always_matches: true`, no WHERE clause needed:**

```bash
curl -X POST http://localhost:8080/api/v1/rulesets/doc_access/filter \
  -d '{ "known_input": { "user": { "role": "admin" } }, "target_results": ["ALLOW"] }'
```

```json
{ "filter": "TRUE", "always_matches": true }
```

**Moderator — only published/review documents:**

```json
{
  "filter": "(status = 'published' OR status = 'review')"
}
```

**Free member alice — owner or public docs only:**

The `subscription = "free"` folds `user.subscription == "premium"` to false, eliminating the premium-tier path.

```json
{
  "filter": "(owner_id = 'alice') OR ((visibility = 'public' AND status = 'published'))"
}
```

**Premium member bob — three paths:**

```json
{
  "filter": "(owner_id = 'bob') OR ((visibility = 'public' AND status = 'published')) OR (tier IN ('free', 'standard'))"
}
```

**Unknown role (guest) — `never_matches: true`:**

```json
{ "filter": null, "never_matches": true }
```

### MongoDB `$match` Format

Use `"format": "mongo"` to get a MongoDB aggregation pipeline `$match` stage. The result is a JSON object you can pass directly to `db.collection.aggregate([{ $match: filter }])`.

**Free member alice:**

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

**Supported operators:**

| Expression                | `$match` output                             |
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
| Multiple paths            | `{ "$or": [...] }`                          |
| Always matches            | `{}` (empty — no filter)                    |
| Never matches             | `{ "$expr": false }`                        |

Regex metacharacters in string literals are automatically escaped.

### JSON Predicate Format

Use `"format": "json"` for a structured predicate tree that ORMs and front-end clients can consume:

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

**Supported node types:**

| Type                          | Key fields          | Meaning                    |
| ----------------------------- | ------------------- | -------------------------- |
| `eq` `ne` `lt` `le` `gt` `ge` | `field`, `value`    | Comparison                 |
| `and` `or`                    | `conditions[]`      | Logical                    |
| `not`                         | `condition`         | Negation                   |
| `in` `not_in`                 | `field`, `values[]` | Set membership             |
| `contains`                    | `field`, `value`    | Substring / array contains |
| `is_null` `not_null`          | `field`             | NULL check                 |
| `starts_with` `ends_with`     | `field`, `value`    | Prefix / suffix            |
| `always`                      | —                   | No filter needed           |
| `never`                       | —                   | Empty result               |

## SQL Generation Reference

| Expression                | SQL                         |
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
| Multiple paths            | `(...) OR (...)`            |

String literals are single-quote escaped (`'` → `''`). LIKE pattern literals additionally escape `!` → `!!`, `%` → `!%`, `_` → `!_` so that wildcards in values are treated literally. Null comparisons use `IS NULL` / `IS NOT NULL` to match SQL three-valued logic. Arithmetic operators and unsupported functions return a `500` error.

## Errors

| Status | Description                                                       |
| ------ | ----------------------------------------------------------------- |
| 400    | `target_results` is empty                                         |
| 404    | Ruleset not found                                                 |
| 500    | Filter compilation failed (e.g. unsupported operator in SQL mode) |

## Known Limitations

- **Action step mutations**: `SetVariable` side-effects are not tracked. Downstream conditions referencing mutated variables are treated as unknown columns — the filter may be a superset. Application-level execution handles the final filtering.
- **Depth limit**: 50 steps maximum traversal depth (hard limit, prevents infinite loops in cyclic graphs).

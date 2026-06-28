# Fact Catalog & Concepts

The fact catalog is the project's unified description of "which fields rules can read." Studio autocomplete, contract validation, and test-case suggestions are all driven by it.

## Why a Catalog

Common pain points without one:

- Rulesets disagree on field names (`user.id` / `customer_id` / `uid`)
- Type drift (a numeric field becomes a string after one release)
- Field semantics live only in engineers' heads

The fact catalog makes these constraints **explicit**: every field has a name, type, description, and example, serving as the project's single source of truth.

## Fact

An atomic field definition.

```jsonc
// POST /api/v1/projects/:pid/facts
{
  "name": "user.age",
  "type": "number",
  "description": "User age in years",
  "example": 28,
  "tags": ["user", "demographic"]
}
```

Supported types: `string` · `number` · `boolean` · `array<T>` · `object` · `concept:<name>`.

## Concept

A composite structure. When multiple rulesets need to refer to the same object (e.g. "User", "Order"), define the concept once and reference it from facts as `concept:User`.

```jsonc
// POST /api/v1/projects/:pid/concepts
{
  "name": "User",
  "fields": [
    { "name": "id", "type": "string" },
    { "name": "age", "type": "number" },
    { "name": "vip", "type": "boolean" }
  ]
}
```

## API

| Operation      | Endpoint                                          |
| -------------- | ------------------------------------------------- |
| List facts     | `GET /api/v1/projects/:pid/facts`                 |
| Create fact    | `POST /api/v1/projects/:pid/facts`                |
| Update/delete  | `PUT/DELETE /api/v1/projects/:pid/facts/:name`    |
| List concepts  | `GET /api/v1/projects/:pid/concepts`              |
| Create concept | `POST /api/v1/projects/:pid/concepts`             |
| Update/delete  | `PUT/DELETE /api/v1/projects/:pid/concepts/:name` |

## Coupling with Contracts & Studio

- [Contracts](./contracts) constrain a ruleset's input and output by referencing facts/concepts.
- Studio expression autocomplete and type checks come from the catalog.
- Test cases use the catalog to suggest input fields — the catalog is where all "type info" for a project converges.

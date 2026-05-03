# Decision Contracts

A decision contract is a ruleset's "type signature": it declares which fields the rule expects to read, which result codes it must emit, and which fields each result carries.

Contracts decouple rules from callers — apps call against the contract, authors write against it, and the platform validates both ends agree before each release.

## Contract Shape

```jsonc
// POST /api/v1/projects/:pid/contracts
{
  "name": "discount-check",
  "version": "1.0.0",
  "input": {
    "fields": [
      { "name": "user.age", "type": "number", "required": true },
      { "name": "user.vip", "type": "boolean", "required": false, "default": false },
      { "name": "order.amount", "type": "number", "required": true }
    ]
  },
  "outputs": [
    { "code": "VIP", "fields": [{ "name": "discount", "type": "number" }] },
    { "code": "NORMAL", "fields": [{ "name": "discount", "type": "number" }] },
    { "code": "DENY", "fields": [{ "name": "reason", "type": "string" }] }
  ]
}
```

## When Validation Runs

| Trigger             | Checks                                                                                 |
| ------------------- | -------------------------------------------------------------------------------------- |
| Studio live editing | Expressions reference fields declared in the contract `input`                          |
| Draft save          | Each Terminal's `code` is in the contract's `outputs` list                             |
| Test run            | Test inputs satisfy the contract's `input.required`                                    |
| Pre-release         | Contract diff against the previous release; breaking changes need higher-tier approval |

## Relationship with the Fact Catalog

The contract's `input.fields` reference names and types from the [fact catalog](./catalog). If a field's type changes in the catalog, contracts that depend on it are flagged as "needs migration."

## API

| Operation     | Endpoint                                           |
| ------------- | -------------------------------------------------- |
| List          | `GET /api/v1/projects/:pid/contracts`              |
| Create        | `POST /api/v1/projects/:pid/contracts`             |
| Update/delete | `PUT/DELETE /api/v1/projects/:pid/contracts/:name` |

## Versioning & Breaking Changes

Contracts carry a `version` field that follows SemVer:

- **patch / minor** — adding optional fields, adding `output.code`, relaxing validation: non-breaking.
- **major** — removing fields, type changes, removing output codes: breaking. Release requests will require an explicit ack.

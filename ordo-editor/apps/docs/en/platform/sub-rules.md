# Sub-Rule Assets

Sub-rules let you extract reusable logic snippets as project-scoped or org-scoped assets. Multiple rulesets can call the same asset through a `SubRule` step.

## Use Cases

- **KYC verification** — different business lines (loans, credit cards, insurance) all need the same identity check.
- **Risk scoring** — a customer-risk algorithm reused by many decisions.
- **Blacklist check** — every user-facing ruleset starts with this.

## Data Model

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

- `bindings` — parameters the caller must pass.
- `outputs` — fields written back to the parent context after the sub-rule ends.

## Reference From a Ruleset

In Studio, drop a `SubRule` node, pick a ref name and version, and configure binding expressions and output mapping:

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

## Inline Snapshot at Publish

When a ruleset is published, the platform **deep-inlines** every referenced sub-rule's current version (BFS resolution) into a self-contained flat RuleSet before delivering it to ordo-server:

- Later edits or deletions of the sub-rule do not affect already-published rulesets.
- Engine execution needs zero extra lookups — no overhead.
- Default call depth limit is 10 (preventing recursion); the platform also runs DFS cycle detection.

## Versioning & Diff

Each sub-rule update creates a new version snapshot:

| Operation  | Endpoint                                                  |
| ---------- | --------------------------------------------------------- |
| List       | `GET  /api/v1/orgs/:oid/projects/:pid/sub-rules`          |
| Get/update | `GET/PUT /api/v1/orgs/:oid/projects/:pid/sub-rules/:name` |
| Org-level  | `/api/v1/orgs/:oid/sub-rules` (cross-project sharing)     |

After updating a sub-rule, the platform lists **every ruleset that references it** — those rulesets need to be republished to pick up the new logic.

# Quick Start

This page walks you through writing a discount rule, running it, adding tests, and calling it over HTTP. You need Node.js 18 or later. No Rust toolchain required.

## 1. Install the CLI

```bash
npm install -g @ordo-engine/cli
ordo --version
```

If you'd rather not install globally, replace `ordo` below with `npx @ordo-engine/cli`.

## 2. Create a rules project

```bash
ordo init my-rules && cd my-rules
```

The project comes with a sample rule, `loan-approval`. Rules live in `rulesets/` and tests in `tests/`.

## 3. Write a rule

Create `rulesets/discount.json`. It picks a discount rate from the membership tier and order amount, then computes the amount to pay:

```json
{
  "config": { "name": "discount", "version": "1.0.0", "entry_step": "pick_rate" },
  "steps": {
    "pick_rate": {
      "id": "pick_rate",
      "name": "Pick discount rate",
      "type": "decision_table",
      "inputs": ["user.tier", "order.amount"],
      "outputs": ["rate"],
      "rules": [
        { "when": ["gold", ">= 1000"], "then": [0.15] },
        { "when": ["gold", "*"],       "then": [0.10] },
        { "when": ["*", ">= 1000"],    "then": [0.05] }
      ],
      "default": [0],
      "next_step": "done"
    },
    "done": {
      "id": "done",
      "name": "Done",
      "type": "terminal",
      "result": {
        "code": "OK",
        "output": [
          ["rate", "$rate"],
          ["pay", "order.amount * (1 - $rate)"]
        ]
      }
    }
  }
}
```

`pick_rate` is a decision table. Rows are matched top to bottom and the first match wins; `*` matches anything, and `default` applies when no row matches. `done` is a terminal step, and `$rate` is the variable the table set. See [Decision Table](./decision-table) for the full syntax.

## 4. Run it

```bash
ordo validate
ordo trace discount --input '{"user":{"tier":"gold"},"order":{"amount":1200}}'
```

```text
code:    OK
output:  {
  "rate": 0.15,
  "pay": 1020.0
}

path:    pick_rate -> done
```

`validate` compiles every rule and reports errors. `trace` runs the rule and lists each step it went through.

## 5. Add tests

Create `tests/discount.json`:

```json
[
  {
    "name": "gold member, large order",
    "input": { "user": { "tier": "gold" }, "order": { "amount": 1200 } },
    "expect": { "code": "OK", "output": { "rate": 0.15, "pay": 1020.0 } }
  },
  {
    "name": "regular member, small order",
    "input": { "user": { "tier": "silver" }, "order": { "amount": 300 } },
    "expect": { "code": "OK", "output": { "rate": 0, "pay": 300 } }
  }
]
```

```bash
ordo test
```

From now on, run `ordo test` after every rule change. Add it to CI and a broken rule can't be merged.

## 6. Call it as a service

Applications call rules through `ordo-server`. Start it with Docker:

```bash
docker run -p 8080:8080 ghcr.io/ordo-engine/ordo:latest
```

Upload the rule, then execute it:

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H 'Content-Type: application/json' \
  -d @rulesets/discount.json

curl -X POST http://localhost:8080/api/v1/execute/discount \
  -H 'Content-Type: application/json' \
  -d '{"input":{"user":{"tier":"gold"},"order":{"amount":1200}}}'
```

```json
{ "code": "OK", "message": "", "output": { "rate": 0.15, "pay": 1020.0 }, "duration_us": 12 }
```

To load rules from a directory and keep version history, see [Rule Persistence](./persistence). For other ways to run the server, see [Install & Run](./getting-started).

## Next steps

- [Rule Structure](./rule-structure): every step type and field
- [Expression Syntax](./expression-syntax): what you can write in conditions and outputs
- [HTTP API](/en/api/http-api): the full API

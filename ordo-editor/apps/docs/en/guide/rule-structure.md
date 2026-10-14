# Rule Structure

A rule file (a ruleset) is JSON or YAML with two parts: `config` and `steps`. Steps form a graph. Execution starts at `entry_step` and ends at a terminal step.

```json
{
  "config": {
    "name": "loan-approval",
    "version": "1.0.0",
    "entry_step": "check_age"
  },
  "steps": {
    "check_age": { "id": "check_age", "name": "Check age", "type": "decision", "...": "..." }
  }
}
```

## config

| Field          | Required | Description                                              |
| -------------- | -------- | -------------------------------------------------------- |
| `name`         | yes      | Ruleset name. Callers use it to run the rule.            |
| `version`      | no       | Version string, default `1.0.0`                          |
| `description`  | no       | Free text                                                |
| `entry_step`   | yes      | ID of the first step                                     |
| `input_schema` | no       | Declared input fields, checked before execution (below)  |
| `timeout_ms`   | no       | Execution timeout, default `5000`                        |
| `field_missing`| no       | `lenient` (default) or `strict`; what a missing field does in a condition |

### Input schema

Without a schema, a missing field makes a condition false. A rule like `applicant.age < 18` is then skipped when `age` is not sent. Declare the fields you need, and bad input is rejected instead:

```json
"input_schema": [
  { "name": "applicant", "type": "object", "required": true, "fields": [
    { "name": "age", "type": "number", "required": true },
    { "name": "credit_score", "type": "number", "required": true }
  ]},
  { "name": "channel", "type": "string", "defaultValue": "online" },
  { "name": "items", "type": "array", "itemType": { "name": "item", "type": "object" } }
]
```

- `type`: `string`, `number`, `decimal` (exact, for money), `boolean`, `array`, `object` or `any`.
- `required`: the field must be present and not `null`.
- `defaultValue`: used when the field is absent or `null`.
- `fields` and `itemType` check nested objects and array items the same way.

All violations are reported together. Over HTTP the response is `400` with code `INVALID_INPUT`, for example `Invalid input: applicant.age: required field is missing`. Over gRPC it is `INVALID_ARGUMENT`.

## Steps

Every step has `id`, `name` and `type`. `id` must match the key in `steps`. `name` is required and shows up in traces.

| `type`           | What it does                                                    |
| ---------------- | --------------------------------------------------------------- |
| `decision`       | Checks conditions in order and follows the first that matches   |
| `decision_table` | Matches table rows against the input and sets variables. See [Decision Tables](./decision-table) |
| `action`         | Runs actions (set a variable, log, call out), then moves on     |
| `sub_rule`       | Runs a reusable sub-graph and copies its results back           |
| `terminal`       | Ends execution and returns a result                             |

### decision

```json
{
  "id": "check_amount",
  "name": "Check order amount",
  "type": "decision",
  "branches": [
    { "condition": "order.amount > 1000", "next_step": "high_value" },
    { "condition": "order.amount > 100", "next_step": "medium_value" }
  ],
  "default_next": "low_value"
}
```

Branches are checked top to bottom and the first true condition wins. `default_next` is used when none match. Conditions are [expressions](./expression-syntax).

### action

```json
{
  "id": "calc",
  "name": "Calculate total",
  "type": "action",
  "actions": [
    { "action": "set_variable", "name": "subtotal", "value": "price * qty" },
    { "action": "set_variable", "name": "discount", "value": "if vip then 0.15 else 0" }
  ],
  "next_step": "done"
}
```

Later steps read a variable as `$subtotal`. The actions are:

| `action`        | Fields                                         | What it does |
| --------------- | ---------------------------------------------- | ------------ |
| `set_variable`  | `name`, `value`                                | Evaluates `value` and stores it as `$name` |
| `log`           | `message`, `level` (`debug`, `info`, `warn`, `error`) | Writes a log line |
| `metric`        | `name`, `value`, `tags`                        | Records a number. ordo-server exports it as the gauge `ordo_rule_<name>` |
| `external_call` | `service`, `method`, `params`, `result_variable`, `timeout_ms` | Calls a capability provided by the host (below) |

`value` is an expression string. The JSON AST form (`{"Binary": {...}}`) is accepted too, and rulesets saved by the server are stored in that form.

#### External calls

On ordo-server, `external_call` can reach these built-in capabilities:

| `service`            | `method`                                   | `params`                                |
| -------------------- | ------------------------------------------ | --------------------------------------- |
| `network.http`       | `GET`, `POST`, `PUT`, `PATCH`, `DELETE` … (upper case) | `url`, optional `headers`, `json_body` |
| `metrics.prometheus` | `gauge`, `counter`                         | `name`, `value`, optional `tags`        |
| `audit.logger`       | `rule_executed`                            | `rule_name`, `duration_us`, `result`    |

```json
{
  "action": "external_call",
  "service": "network.http",
  "method": "GET",
  "params": [["url", "\"https://api.example.com/score?id=\" + user.id"]],
  "result_variable": "score",
  "timeout_ms": 1000
}
```

The response is stored under `$score`: `$score.payload.status`, `$score.payload.body`, and `$score.payload.json_body` when the body is JSON. Retries and fallbacks are not built in. Handle a failed call with a decision step after it. For a custom provider, see [`examples/capability-demo`](https://github.com/Ordo-Engine/Ordo/tree/main/examples/capability-demo).

### sub_rule

Sub-graphs live in `sub_rules` at the top level of the file. A `sub_rule` step binds inputs into it and copies variables back out:

```json
{
  "config": { "name": "loan", "version": "1.0.0", "entry_step": "risk" },
  "sub_rules": {
    "risk_score": {
      "entry_step": "score",
      "steps": {
        "score": { "id": "score", "name": "Score", "type": "action",
          "actions": [{ "action": "set_variable", "name": "score", "value": "if age < 25 then 80 else 20" }],
          "next_step": "end" },
        "end": { "id": "end", "name": "End", "type": "terminal", "result": { "code": "DONE" } }
      }
    }
  },
  "steps": {
    "risk": {
      "id": "risk", "name": "Risk score", "type": "sub_rule",
      "ref_name": "risk_score",
      "bindings": [["age", "applicant.age"]],
      "outputs": [["risk", "score"]],
      "next_step": "done"
    },
    "done": { "id": "done", "name": "Done", "type": "terminal",
      "result": { "code": "OK", "output": [["risk", "$risk"]] } }
  }
}
```

`bindings` are `[child_field, parent_expression]` pairs. `outputs` are `[parent_variable, child_variable]` pairs.

### terminal

```json
{
  "id": "approved",
  "name": "Approved",
  "type": "terminal",
  "result": {
    "code": "APPROVED",
    "message": "Application approved",
    "output": [["limit", "applicant.income * 0.3"]],
    "data": { "tier": "standard" }
  }
}
```

| Field     | Description                                               |
| --------- | --------------------------------------------------------- |
| `code`    | Result code. Callers branch on it.                        |
| `message` | Optional text                                             |
| `output`  | `[name, expression]` pairs, evaluated at the end          |
| `data`    | Fixed values, merged into the output                      |

## A full example

```json
{
  "config": { "name": "loan-approval", "version": "1.0.0", "entry_step": "check_age" },
  "steps": {
    "check_age": {
      "id": "check_age", "name": "Check age", "type": "decision",
      "branches": [{ "condition": "applicant.age < 18", "next_step": "reject" }],
      "default_next": "check_income"
    },
    "check_income": {
      "id": "check_income", "name": "Check income", "type": "decision",
      "branches": [{ "condition": "applicant.income >= 30000", "next_step": "approve" }],
      "default_next": "reject"
    },
    "approve": {
      "id": "approve", "name": "Approve", "type": "terminal",
      "result": { "code": "APPROVED", "output": [["max_amount", "applicant.income * 2"]] }
    },
    "reject": {
      "id": "reject", "name": "Reject", "type": "terminal",
      "result": { "code": "REJECTED" }
    }
  }
}
```

The same file in YAML works the same way. Run `ordo validate` to compile every step and check that all `next_step` targets exist.

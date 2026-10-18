# Decision Tables

A decision table is a step of type `decision_table`. Each row is a rule: input cells on the left, output values on the right. It is often shorter and easier to review than a chain of decision steps.

```json
{
  "id": "discount",
  "name": "Discount",
  "type": "decision_table",
  "hit_policy": "first",
  "inputs": ["customer.tier", "order.amount"],
  "outputs": ["discount", "reason"],
  "rules": [
    { "when": ["gold", ">= 1000"], "then": [0.15, "Gold, large order"] },
    { "when": [["silver", "bronze"], "100..999"], "then": [0.05, "= customer.tier + \" tier\""] }
  ],
  "default": [0, "No discount"],
  "next_step": "price"
}
```

- `inputs` are expressions, one per column.
- `outputs` are variable names. Later steps read them as `$discount` and `$reason`.
- Rows are checked top to bottom.

The same table in YAML:

```yaml
discount:
  id: discount
  name: Discount
  type: decision_table
  inputs: [customer.tier, order.amount]
  outputs: [discount, reason]
  rules:
    - when: [gold, ">= 1000"]
      then: [0.15, "Gold, large order"]
    - when: [[silver, bronze], "100..999"]
      then: [0.05, "= customer.tier + \" tier\""]
  default: [0, "No discount"]
  next_step: price
```

## Input cells

| Cell                               | Matches when                                |
| ---------------------------------- | ------------------------------------------- |
| `"*"`, `"-"`, `""` or `null`       | always                                      |
| `"gold"`, `42`, `true`             | the input equals the value                  |
| `["silver", "bronze"]`             | the input is one of the values              |
| `">= 1000"`, `"< 18"`, `"!= \"x\""` | the comparison holds                       |
| `"in [...]"`, `"not in [...]"`     | membership                                  |
| `"18..65"`                         | the number is in the range, ends included   |
| `"= <expression>"`                 | the expression is true (it can use any field) |

A plain string is compared as text. To compare with a string that starts with an operator or with `in `, write `"== \"in stock\""`.

## Output cells

A literal (`0.15`, `"Gold"`, `null`, an array or an object), or `"= <expression>"` to compute it, such as `"= order.amount * 0.1"`.

## Hit policies

| `hit_policy`      | Result |
| ----------------- | ------ |
| `first` (default) | The first matching row sets the outputs. With no match, `default` is used. Without a `default`, execution fails instead of going on with missing values. |
| `collect`         | Every matching row, in order. Each output becomes an array, or a single value when `aggregate` is `sum`, `count`, `min` or `max`. |

With `collect` and no match, outputs are `[]`, `sum` and `count` are `0`, and `min` and `max` are `null`.

Adding up risk points from every row that matches:

```json
{
  "id": "risk",
  "name": "Risk points",
  "type": "decision_table",
  "hit_policy": "collect",
  "aggregate": "sum",
  "inputs": ["order.amount"],
  "outputs": ["points"],
  "rules": [
    { "when": ["> 100"], "then": [10] },
    { "when": ["> 500"], "then": [20] }
  ],
  "next_step": "decide"
}
```

For an amount of 600, `$points` is 30.

Studio also has a table view for editing rules as a table. See [Studio](/en/platform/studio).

Decision table steps cannot be compiled to the binary `.ordo` format yet.

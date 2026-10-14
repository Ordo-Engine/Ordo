# Expressions

Conditions, `set_variable` values and terminal outputs are all expressions. The language is small: no loops, no assignments, no side effects.

You can try any expression with the CLI:

```bash
ordo eval 'order.amount >= 1000 && user.tier == "gold"' \
  --input '{"order":{"amount":1200},"user":{"tier":"gold"}}'
```

## Values

| Kind    | Examples                         |
| ------- | -------------------------------- |
| Number  | `42`, `3.14`                     |
| String  | `"gold"`, `'gold'`               |
| Boolean | `true`, `false`                  |
| Null    | `null`                           |
| Array   | `[1, 2, 3]`, `["a", "b"]`        |
| Object  | `{"a": 1}`                       |

## Fields and variables

```
user.name               # a field of the input
order.items.0.price     # array element by index
$subtotal               # a variable set by an earlier step
```

A missing field makes a condition false by default. To reject such input instead, declare an [input schema](./rule-structure#input-schema) or set `field_missing: strict`.

## Operators

| Operator                        | Meaning                       |
| ------------------------------- | ----------------------------- |
| `+ - * / %`                     | Arithmetic. `+` also joins strings |
| `== != < <= > >=`               | Comparison                    |
| `&&` or `and`, `\|\|` or `or`    | Logic                         |
| `!` or `not`                    | Negation                      |
| `x in [..]`, `x not in [..]`    | Membership                    |
| `s contains "sub"`, `arr contains x` | Substring or array item  |

```
age >= 18 && status == "active"
tier in ["gold", "platinum"]
email contains "@example.com"
```

Binding, from loosest to tightest: `||`, `&&`, comparisons, `in` / `not in` / `contains`, `+ -`, `* / %`. Use parentheses when in doubt.

## Numbers

- Numbers compare by value: `3 == 3.0` is true.
- Division does not truncate: `10 / 4` is `2.5`, `10 / 5` is `2`. Use `floor(a / b)` for integer division.
- Strings and numbers are not converted for you: `"100" > 50` is an error. Use `to_int(...)` or `to_float(...)`.
- For money, use `decimal(...)` or declare the field as `decimal` in the input schema. `decimal(0.1) + 0.2 == 0.3` is true. Arithmetic with a decimal stays decimal.

## Conditionals

```
if tier == "gold" then 0.20 else if tier == "silver" then 0.10 else 0.05
```

## Missing values

```
exists(user.discount)                       # true if the field is there and not null
coalesce(user.nickname, user.name, "Guest") # first value that is not null
```

## Functions

```
len(items) > 0
round(price * 0.85, 2)
starts_with(order_id, "ORD-")
```

See [Built-in Functions](./builtin-functions) for the full list.

---
name: ordo-expression-syntax
description: Ordo expression syntax quick reference. Includes comparison operators, logical operators, built-in functions, conditional expressions, field access syntax. Use for writing rule conditions, calculating output values, data transformation.
---

# Ordo Expression Syntax

## Operators

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `status == "active"` |
| `!=` | Not equal | `status != "banned"` |
| `>` | Greater than | `age > 18` |
| `>=` | Greater or equal | `balance >= 1000` |
| `<` | Less than | `price < 100` |
| `<=` | Less or equal | `level <= 5` |

### Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `&&` | And | `age >= 18 && active == true` |
| `\|\|` | Or | `vip == true \|\| points > 1000` |
| `!` | Not | `!is_blocked` |

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `price + tax` |
| `-` | Subtraction | `total - discount` |
| `*` | Multiplication | `quantity * price` |
| `/` | Division | `total / count` |
| `%` | Modulo | `index % 2` |

## Field Access

### Dot Notation

```
user.name
user.profile.level
order.items[0].price
```

### Array Indexing

```
items[0]
matrix[1][2]
users[index]
```

### Safe Access (coalesce)

```
coalesce(user.nickname, user.name, "Anonymous")
```

## Built-in Functions

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| `len(s)` | String length | `len(name) > 0` |
| `upper(s)` | To uppercase | `upper(code) == "VIP"` |
| `lower(s)` | To lowercase | `lower(email)` |
| `trim(s)` | Trim whitespace | `trim(input)` |
| `starts_with(s, prefix)` | Prefix match | `starts_with(url, "https")` |
| `ends_with(s, suffix)` | Suffix match | `ends_with(file, ".pdf")` |
| `contains_str(s, sub)` | Contains substring | `contains_str(desc, "urgent")` |
| `substring(s, start, end)` | Extract substring | `substring(code, 0, 3)` |

### Math Functions

| Function | Description | Example |
|----------|-------------|---------|
| `abs(n)` | Absolute value | `abs(diff) < 0.01` |
| `min(a, b, ...)` | Minimum | `min(price, max_price)` |
| `max(a, b, ...)` | Maximum | `max(0, balance)` |
| `floor(n)` | Floor | `floor(rating)` |
| `ceil(n)` | Ceiling | `ceil(price)` |
| `round(n)` | Round | `round(average)` |

### Array Functions

| Function | Description | Example |
|----------|-------------|---------|
| `len(arr)` | Array length | `len(items) > 0` |
| `sum(arr)` | Sum of array | `sum(prices) >= 100` |
| `avg(arr)` | Average | `avg(scores) > 60` |
| `count(arr)` | Element count | `count(orders)` |
| `first(arr)` | First element | `first(results)` |
| `last(arr)` | Last element | `last(history)` |

### Type Functions

| Function | Description | Example |
|----------|-------------|---------|
| `type(v)` | Get type | `type(value) == "string"` |
| `is_null(v)` | Is null | `is_null(optional)` |
| `is_number(v)` | Is number | `is_number(input)` |
| `is_string(v)` | Is string | `is_string(data)` |
| `is_array(v)` | Is array | `is_array(items)` |

### Conversion Functions

| Function | Description | Example |
|----------|-------------|---------|
| `to_int(v)` | To integer | `to_int(str_num)` |
| `to_float(v)` | To float | `to_float(price)` |
| `to_string(v)` | To string | `to_string(code)` |

### Time Functions

| Function | Description | Example |
|----------|-------------|---------|
| `now()` | Current timestamp (seconds) | `now() - created_at > 86400` |
| `now_millis()` | Current timestamp (milliseconds) | `now_millis()` |

## Conditional Expressions

### if-then-else

```
if condition then value1 else value2
```

Examples:

```
if exists(discount) then price * (1 - discount) else price
if vip == true then 0.2 else 0.05
if age >= 18 then "adult" else "minor"
```

### exists Check

```
if exists(user.nickname) then user.nickname else user.name
```

### Nested Conditions

```
if tier == "gold" then 0.2
else if tier == "silver" then 0.1
else 0.05
```

## Literals

### Numbers

```
42          # Integer
3.14        # Float
-100        # Negative
```

### Strings

```
"hello"
"it's ok"
"line1\nline2"
```

### Booleans

```
true
false
```

### Null

```
null
```

## Rust API

### Parsing Expressions

```rust
use ordo_core::prelude::*;

let parser = ExprParser::new();
let expr = parser.parse("age >= 18 && status == \"active\"")?;
```

### Evaluation

```rust
let evaluator = Evaluator::new();
let context = Context::from_json(r#"{"age": 25, "status": "active"}"#)?;
let result = evaluator.eval(&expr, &context)?;
```

### Compile to Bytecode

```rust
let compiler = ExprCompiler::new();
let compiled = compiler.compile(&expr)?;
let vm = BytecodeVM::new();
let result = vm.execute(&compiled, &context)?;
```

## Key Files

- `crates/ordo-core/src/expr/parser.rs` - Expression parser
- `crates/ordo-core/src/expr/eval.rs` - Expression evaluation
- `crates/ordo-core/src/expr/functions.rs` - Built-in functions
- `crates/ordo-core/src/expr/ast.rs` - AST definitions

# Expression Syntax

Ordo uses a powerful expression language for defining conditions in rules.

## Basic Syntax

### Comparisons

```
age >= 18
status == "active"
score != 0
amount < 1000
count <= 10
level > 5
```

### Logical Operators

```
# AND
age >= 18 && status == "active"

# OR
tier == "gold" || tier == "platinum"

# NOT
!is_blocked

# Combined
(age >= 18 && income > 30000) || has_cosigner == true
```

### Arithmetic

```
price * quantity
total - discount
(base + bonus) * multiplier
amount / 100
count % 2
```

## Field Access

### Object Properties

```
user.name
order.items.count
profile.settings.notifications
```

### Array Indexing

```
items[0]
items[0].price
users[1].name
```

### Nested Access

```
data.users[0].profile.email
order.items[2].product.category
```

## Built-in Functions

### String Functions

| Function                 | Description       | Example                    |
| ------------------------ | ----------------- | -------------------------- |
| `len(s)`                 | String length     | `len(name) > 0`            |
| `upper(s)`               | Uppercase         | `upper(code) == "VIP"`     |
| `lower(s)`               | Lowercase         | `lower(email)`             |
| `trim(s)`                | Remove whitespace | `trim(input)`              |
| `contains(s, sub)`       | Check substring   | `contains(email, "@")`     |
| `starts_with(s, prefix)` | Check prefix      | `starts_with(code, "PRE")` |
| `ends_with(s, suffix)`   | Check suffix      | `ends_with(file, ".pdf")`  |

### Array Functions

| Function   | Description    | Example              |
| ---------- | -------------- | -------------------- |
| `len(arr)` | Array length   | `len(items) > 0`     |
| `sum(arr)` | Sum of numbers | `sum(prices) >= 100` |
| `avg(arr)` | Average        | `avg(scores) > 70`   |
| `min(arr)` | Minimum value  | `min(bids)`          |
| `max(arr)` | Maximum value  | `max(scores) == 100` |

### Numeric Functions

| Function   | Description      | Example              |
| ---------- | ---------------- | -------------------- |
| `abs(n)`   | Absolute value   | `abs(balance) < 100` |
| `round(n)` | Round to nearest | `round(price)`       |
| `floor(n)` | Round down       | `floor(score)`       |
| `ceil(n)`  | Round up         | `ceil(amount)`       |

### Utility Functions

| Function              | Description           | Example                             |
| --------------------- | --------------------- | ----------------------------------- |
| `exists(field)`       | Check if field exists | `exists(discount)`                  |
| `coalesce(a, b, ...)` | First non-null value  | `coalesce(nickname, name, "Guest")` |

## Conditional Expressions

```
# If-then-else
if exists(discount) then price * (1 - discount) else price

# Nested conditionals
if tier == "gold" then 0.20
else if tier == "silver" then 0.10
else 0.05
```

## Type Coercion

Ordo automatically handles type conversions:

```
# String to number comparison
"100" > 50  # true (string converted to number)

# Boolean expressions
1 == true   # true
0 == false  # true
```

## Examples

### Complex Discount Calculation

```
# VIP users get 20%, others get 5%
if user.vip == true then order.total * 0.80 else order.total * 0.95
```

### Risk Score Evaluation

```
# Check multiple risk factors
credit_score >= 700 && debt_ratio < 0.4 && employment_years >= 2
```

### Order Validation

```
# Validate order has items and meets minimum
len(order.items) > 0 && sum(order.items.*.price) >= 10
```

### Null-safe Field Access

```
# Use coalesce for optional fields
coalesce(user.preferred_name, user.name, "Customer")
```

### Array Aggregation

```
# Check if any item exceeds limit
max(order.items.*.quantity) <= 100

# Validate total
sum(cart.items.*.price) <= user.credit_limit
```

## Operator Precedence

From highest to lowest:

1. `()` - Parentheses
2. `!` - NOT
3. `*`, `/`, `%` - Multiplication, Division, Modulo
4. `+`, `-` - Addition, Subtraction
5. `<`, `<=`, `>`, `>=` - Comparisons
6. `==`, `!=` - Equality
7. `&&` - AND
8. `||` - OR

## Best Practices

1. **Use parentheses for clarity**: `(a && b) || c` is clearer than `a && b || c`
2. **Check for existence**: Use `exists(field)` before accessing optional fields
3. **Use coalesce for defaults**: `coalesce(value, default)` instead of complex conditionals
4. **Keep expressions simple**: Break complex logic into multiple steps
5. **Test edge cases**: Empty arrays, null values, missing fields

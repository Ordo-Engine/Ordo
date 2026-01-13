# Built-in Functions

Ordo provides a comprehensive set of built-in functions for use in expressions.

## String Functions

### len(string)

Returns the length of a string.

```
len("hello")        # 5
len("")             # 0
len(user.name)      # depends on name
```

### upper(string)

Converts string to uppercase.

```
upper("hello")      # "HELLO"
upper(code)         # uppercase of code
```

### lower(string)

Converts string to lowercase.

```
lower("HELLO")      # "hello"
lower(email)        # lowercase email
```

### trim(string)

Removes leading and trailing whitespace.

```
trim("  hello  ")   # "hello"
trim(input)         # trimmed input
```

### contains(string, substring)

Checks if string contains substring.

```
contains("hello world", "world")    # true
contains(email, "@gmail.com")       # true if Gmail
```

### starts_with(string, prefix)

Checks if string starts with prefix.

```
starts_with("hello", "he")          # true
starts_with(order_id, "ORD-")       # true if starts with ORD-
```

### ends_with(string, suffix)

Checks if string ends with suffix.

```
ends_with("hello", "lo")            # true
ends_with(filename, ".pdf")         # true if PDF file
```

## Array Functions

### len(array)

Returns the number of elements in an array.

```
len([1, 2, 3])          # 3
len(order.items)        # number of items
len([])                 # 0
```

### sum(array)

Returns the sum of numeric array elements.

```
sum([1, 2, 3])          # 6
sum(prices)             # total of prices
sum([])                 # 0
```

### avg(array)

Returns the average of numeric array elements.

```
avg([1, 2, 3])          # 2
avg(scores)             # average score
avg([10])               # 10
```

### min(array)

Returns the minimum value in an array.

```
min([3, 1, 2])          # 1
min(bids)               # lowest bid
```

### max(array)

Returns the maximum value in an array.

```
max([3, 1, 2])          # 3
max(scores)             # highest score
```

## Numeric Functions

### abs(number)

Returns the absolute value.

```
abs(-5)                 # 5
abs(5)                  # 5
abs(balance)            # positive balance
```

### round(number)

Rounds to the nearest integer.

```
round(3.4)              # 3
round(3.5)              # 4
round(3.6)              # 4
```

### floor(number)

Rounds down to the nearest integer.

```
floor(3.9)              # 3
floor(3.1)              # 3
floor(-3.1)             # -4
```

### ceil(number)

Rounds up to the nearest integer.

```
ceil(3.1)               # 4
ceil(3.9)               # 4
ceil(-3.9)              # -3
```

## Utility Functions

### exists(field)

Checks if a field exists and is not null.

```
exists(user.email)              # true if email exists
exists(optional_field)          # false if null/missing
```

Use this to safely check optional fields before accessing them:

```
exists(discount) && discount > 0
```

### coalesce(value1, value2, ...)

Returns the first non-null value.

```
coalesce(null, "default")           # "default"
coalesce(user.nickname, user.name)  # nickname or name
coalesce(a, b, c, "fallback")       # first non-null
```

Common patterns:

```
# Default value
coalesce(config.timeout, 30)

# Fallback chain
coalesce(user.display_name, user.full_name, user.email, "Anonymous")

# Safe field access
coalesce(response.data.value, 0)
```

## Function Composition

Functions can be composed:

```
# Uppercase and check length
len(upper(name)) > 0

# Sum and compare
sum(prices) > avg(prices) * len(prices)

# Coalesce with transformation
upper(coalesce(code, "DEFAULT"))
```

## Type Handling

Functions handle type conversions automatically:

```
# len works on strings and arrays
len("hello")        # 5
len([1, 2, 3])      # 3

# Numeric functions convert strings
abs("-5")           # 5
sum(["1", "2"])     # 3
```

## Error Handling

Functions return sensible defaults on errors:

| Scenario               | Result |
| ---------------------- | ------ |
| `len(null)`            | 0      |
| `sum([])`              | 0      |
| `avg([])`              | 0      |
| `upper(null)`          | ""     |
| `coalesce(null, null)` | null   |

## Performance Notes

- All functions are optimized for high performance
- String operations use zero-copy where possible
- Array functions use iterators (no intermediate allocations)
- Function calls add ~10-50ns overhead

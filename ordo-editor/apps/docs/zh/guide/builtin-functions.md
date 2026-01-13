# 内置函数

Ordo 提供了一套全面的内置函数供在表达式中使用。

## 字符串函数

### len(string)

返回字符串的长度。

```
len("hello")        # 5
len("")             # 0
len(user.name)      # 取决于 name 的长度
```

### upper(string)

将字符串转换为大写。

```
upper("hello")      # "HELLO"
upper(code)         # code 的大写形式
```

### lower(string)

将字符串转换为小写。

```
lower("HELLO")      # "hello"
lower(email)        # 邮箱的小写形式
```

### trim(string)

移除首尾的空白字符。

```
trim("  hello  ")   # "hello"
trim(input)         # 去除空格后的 input
```

### contains(string, substring)

检查字符串是否包含子字符串。

```
contains("hello world", "world")    # true
contains(email, "@gmail.com")       # 如果是 Gmail 则为 true
```

### starts_with(string, prefix)

检查字符串是否以指定前缀开头。

```
starts_with("hello", "he")          # true
starts_with(order_id, "ORD-")       # 如果以 ORD- 开头则为 true
```

### ends_with(string, suffix)

检查字符串是否以指定后缀结尾。

```
ends_with("hello", "lo")            # true
ends_with(filename, ".pdf")         # 如果是 PDF 文件则为 true
```

## 数组函数

### len(array)

返回数组中元素的数量。

```
len([1, 2, 3])          # 3
len(order.items)        # 项目数量
len([])                 # 0
```

### sum(array)

返回数值数组元素的总和。

```
sum([1, 2, 3])          # 6
sum(prices)             # 价格总和
sum([])                 # 0
```

### avg(array)

返回数值数组元素的平均值。

```
avg([1, 2, 3])          # 2
avg(scores)             # 平均分数
avg([10])               # 10
```

### min(array)

返回数组中的最小值。

```
min([3, 1, 2])          # 1
min(bids)               # 最低出价
```

### max(array)

返回数组中的最大值。

```
max([3, 1, 2])          # 3
max(scores)             # 最高分数
```

## 数值函数

### abs(number)

返回绝对值。

```
abs(-5)                 # 5
abs(5)                  # 5
abs(balance)            # 正数余额
```

### round(number)

四舍五入到最近的整数。

```
round(3.4)              # 3
round(3.5)              # 4
round(3.6)              # 4
```

### floor(number)

向下取整到最近的整数。

```
floor(3.9)              # 3
floor(3.1)              # 3
floor(-3.1)             # -4
```

### ceil(number)

向上取整到最近的整数。

```
ceil(3.1)               # 4
ceil(3.9)               # 4
ceil(-3.9)              # -3
```

## 工具函数

### exists(field)

检查字段是否存在且不为 null。

```
exists(user.email)              # 如果 email 存在则为 true
exists(optional_field)          # 如果为 null 或缺失则为 false
```

使用它在访问可选字段之前安全地进行检查：

```
exists(discount) && discount > 0
```

### coalesce(value1, value2, ...)

返回第一个非 null 值。

```
coalesce(null, "default")           # "default"
coalesce(user.nickname, user.name)  # nickname 或 name
coalesce(a, b, c, "fallback")       # 第一个非 null 值
```

常见模式：

```
# 默认值
coalesce(config.timeout, 30)

# 回退链
coalesce(user.display_name, user.full_name, user.email, "Anonymous")

# 安全的字段访问
coalesce(response.data.value, 0)
```

## 函数组合

函数可以组合使用：

```
# 大写并检查长度
len(upper(name)) > 0

# 求和并比较
sum(prices) > avg(prices) * len(prices)

# 合并并转换
upper(coalesce(code, "DEFAULT"))
```

## 类型处理

函数会自动处理类型转换：

```
# len 适用于字符串和数组
len("hello")        # 5
len([1, 2, 3])      # 3

# 数值函数会转换字符串
abs("-5")           # 5
sum(["1", "2"])     # 3
```

## 错误处理

函数在出错时会返回合理的默认值：

| 场景                   | 结果 |
| ---------------------- | ---- |
| `len(null)`            | 0    |
| `sum([])`              | 0    |
| `avg([])`              | 0    |
| `upper(null)`          | ""   |
| `coalesce(null, null)` | null |

## 性能说明

- 所有函数都针对高性能进行了优化
- 字符串操作尽可能使用零拷贝
- 数组函数使用迭代器（无中间分配）
- 函数调用增加约 10-50ns 的开销

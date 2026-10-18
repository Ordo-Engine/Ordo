# 表达式语法

条件、输出和赋值里写的都是表达式。本页列出表达式支持的语法。

## 基本语法

### 比较 (Comparisons)

```
age >= 18
status == "active"
score != 0
amount < 1000
count <= 10
level > 5
```

### 逻辑运算符 (Logical Operators)

```
# AND
age >= 18 && status == "active"

# OR
tier == "gold" || tier == "platinum"

# NOT
!is_blocked

# 组合
(age >= 18 && income > 30000) || has_cosigner == true
```

### 算术 (Arithmetic)

```
price * quantity
total - discount
(base + bonus) * multiplier
amount / 100
count % 2
```

数字按数值比较：`3 == 3.0` 为 `true`，调用方传 `3` 还是 `3.0` 结果一致。除法不会静默截断：`10 / 4` 得 `2.5`，`10 / 5` 仍是整数 `2`。需要整除时用 `floor(a / b)`。

涉及金额时用精确小数（decimal）：`decimal(0.1) + 0.2 == 0.3` 为 `true`（普通浮点数是 `false`）。用 `decimal(...)` 包一下金额，或在规则的 `input_schema` 里把输入字段声明为 `"type": "decimal"`。只要有一个操作数是 decimal，运算结果就是 decimal；`round(total, 2)` 结果仍是 decimal。

## 字段访问

### 对象属性 (Object Properties)

```
user.name
order.items.count
profile.settings.notifications
```

### 数组元素

把下标当作路径的一段：

```
items.0
items.0.price
users.1.name
```

## 内置函数

### 字符串函数 (String Functions)

| 函数                     | 描述         | 示例                       |
| ------------------------ | ------------ | -------------------------- |
| `len(s)`                 | 字符串长度   | `len(name) > 0`            |
| `upper(s)`               | 大写         | `upper(code) == "VIP"`     |
| `lower(s)`               | 小写         | `lower(email)`             |
| `trim(s)`                | 去除空格     | `trim(input)`              |
| `contains_str(s, sub)`   | 检查子字符串 | `contains_str(email, "@")` |
| `starts_with(s, prefix)` | 检查前缀     | `starts_with(code, "PRE")` |
| `ends_with(s, suffix)`   | 检查后缀     | `ends_with(file, ".pdf")`  |

### 数组函数 (Array Functions)

| 函数       | 描述     | 示例                 |
| ---------- | -------- | -------------------- |
| `len(arr)` | 数组长度 | `len(items) > 0`     |
| `sum(arr)` | 数值总和 | `sum(prices) >= 100` |
| `avg(arr)` | 平均值   | `avg(scores) > 70`   |
| `min(arr)` | 最小值   | `min(bids)`          |
| `max(arr)` | 最大值   | `max(scores) == 100` |

### 数值函数 (Numeric Functions)

| 函数       | 描述     | 示例                 |
| ---------- | -------- | -------------------- |
| `abs(n)`   | 绝对值   | `abs(balance) < 100` |
| `round(n)` | 四舍五入 | `round(price)`       |
| `round(n, digits)` | 保留小数位 | `round(price * 0.85, 2)` |
| `floor(n)` | 向下取整 | `floor(score)`       |
| `ceil(n)`  | 向上取整 | `ceil(amount)`       |

### 工具函数 (Utility Functions)

| 函数                  | 描述             | 示例                                |
| --------------------- | ---------------- | ----------------------------------- |
| `exists(field)`       | 检查字段是否存在 | `exists(discount)`                  |
| `coalesce(a, b, ...)` | 第一个非空值     | `coalesce(nickname, name, "Guest")` |

## 条件表达式

```
# If-then-else
if exists(discount) then price * (1 - discount) else price

# 嵌套条件
if tier == "gold" then 0.20
else if tier == "silver" then 0.10
else 0.05
```

## 类型

整数和浮点数可以直接比较，其他类型不会自动转换：

```
10 == 10.0           # true
"100" > 50           # 报错：字符串不能和整数比较
to_int("100") > 50   # true
1 == true            # false
```

## 字符串

`\n`、`\t`、`\r`、`\\`、`\"`、`\'` 是转义。其他反斜杠原样保留，所以正则按写的意思生效：
`regex_match("^\d+\.\d{2}$", amount_text)`。

## 示例

### 复杂折扣计算

```
# VIP 用户获得 20% 折扣，其他用户获得 5%
if user.vip == true then order.total * 0.80 else order.total * 0.95
```

### 风险评分评估

```
# 检查多个风险因素
credit_score >= 700 && debt_ratio < 0.4 && employment_years >= 2
```

### 订单验证

```
# 验证订单包含项目且满足最低金额
len(order.items) > 0 && sum(map_extract(order.items, "price")) >= 10
```

### 空安全字段访问

```
# 使用 coalesce 处理可选字段
coalesce(user.preferred_name, user.name, "Customer")
```

### 数组聚合

```
# 检查是否有任何项目的数量超过限制
max(map_extract(order.items, "quantity")) <= 100

# 验证总额
sum(map_extract(cart.items, "price")) <= user.credit_limit
```

## 运算符优先级

从高到低：

1. `()` - 括号
2. `!` - NOT
3. `*`, `/`, `%` - 乘法, 除法, 取模
4. `+`, `-` - 加法, 减法
5. `<`, `<=`, `>`, `>=` - 比较
6. `==`, `!=` - 相等性
7. `&&` - AND
8. `||` - OR

## 最佳实践

1.  **使用括号提高清晰度**：`(a && b) || c` 比 `a && b || c` 更清晰
2.  **检查存在性**：访问可选字段前使用 `exists(field)`
3.  **使用 coalesce 设置默认值**：用 `coalesce(value, default)` 代替复杂的条件判断
4.  **保持表达式简单**：将复杂逻辑拆分为多个步骤
5.  **测试边缘情况**：空数组、null 值、缺失字段

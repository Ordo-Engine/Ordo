# 表达式语法

Ordo 使用强大的表达式语言来定义规则中的条件。

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

## 字段访问

### 对象属性 (Object Properties)

```
user.name
order.items.count
profile.settings.notifications
```

### 数组索引 (Array Indexing)

```
items[0]
items[0].price
users[1].name
```

### 嵌套访问 (Nested Access)

```
data.users[0].profile.email
order.items[2].product.category
```

## 内置函数

### 字符串函数 (String Functions)

| 函数                     | 描述         | 示例                       |
| ------------------------ | ------------ | -------------------------- |
| `len(s)`                 | 字符串长度   | `len(name) > 0`            |
| `upper(s)`               | 大写         | `upper(code) == "VIP"`     |
| `lower(s)`               | 小写         | `lower(email)`             |
| `trim(s)`                | 去除空格     | `trim(input)`              |
| `contains(s, sub)`       | 检查子字符串 | `contains(email, "@")`     |
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

## 类型强制转换

Ordo 自动处理类型转换：

```
# 字符串转数字比较
"100" > 50  # true (字符串被转换为数字)

# 布尔表达式
1 == true   # true
0 == false  # true
```

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
len(order.items) > 0 && sum(order.items.*.price) >= 10
```

### 空安全字段访问

```
# 使用 coalesce 处理可选字段
coalesce(user.preferred_name, user.name, "Customer")
```

### 数组聚合

```
# 检查是否有任何项目的数量超过限制
max(order.items.*.quantity) <= 100

# 验证总额
sum(cart.items.*.price) <= user.credit_limit
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

# 表达式

条件、`set_variable` 的值和终止步骤的输出都是表达式。这门语言很小：没有循环，没有赋值，不能修改外部状态。

任何表达式都可以用命令行试：

```bash
ordo eval 'order.amount >= 1000 && user.tier == "gold"' \
  --input '{"order":{"amount":1200},"user":{"tier":"gold"}}'
```

## 值

| 类型   | 例子                        |
| ------ | --------------------------- |
| 数字   | `42`、`3.14`                |
| 字符串 | `"gold"`、`'gold'`          |
| 布尔   | `true`、`false`             |
| 空值   | `null`                      |
| 数组   | `[1, 2, 3]`、`["a", "b"]`   |
| 对象   | `{"a": 1}`                  |

## 字段和变量

```
user.name               # 输入里的字段
order.items.0.price     # 按下标取数组元素
$subtotal               # 前面步骤设置的变量
$data.limits.vip_min    # 服务端或 `ordo exec --data` 提供的外部数据
```

默认情况下，缺字段只让它所在的那一项为假，条件的其余部分照常生效：没传 age 时，`age < 18 || blacklisted` 遇到黑名单用户仍然为真。想直接拒绝这样的输入，就声明[输入校验](./rule-structure#输入校验)，或者设置 `field_missing: strict`。

## 运算符

| 运算符                          | 含义                         |
| ------------------------------- | ---------------------------- |
| `+ - * / %`                     | 算术。`+` 也能拼接字符串     |
| `== != < <= > >=`               | 比较                         |
| `&&` 或 `and`，`\|\|` 或 `or`    | 逻辑                         |
| `!` 或 `not`                    | 取反                         |
| `x in [..]`、`x not in [..]`    | 是否在列表里                 |
| `s contains "sub"`、`arr contains x` | 包含子串或数组元素      |

```
age >= 18 && status == "active"
tier in ["gold", "platinum"]
email contains "@example.com"
```

结合从松到紧：`||`、`&&`、比较、`in` / `not in` / `contains`、`+ -`、`* / %`。拿不准时加括号。

## 数字

- 数字按值比较：`3 == 3.0` 为真。
- 除法不截断：`10 / 4` 是 `2.5`，`10 / 5` 是 `2`。要整除就写 `floor(a / b)`。
- 字符串和数字不会自动转换：`"100" > 50` 会报错。用 `to_int(...)` 或 `to_float(...)`。
- 金额用 `decimal(...)`，或在输入校验里把字段声明为 `decimal`。`decimal(0.1) + 0.2 == 0.3` 为真。有 decimal 参与的运算结果仍是 decimal。

## 字符串

`\n`、`\t`、`\r`、`\\`、`\"`、`\'` 是转义。其他反斜杠原样保留，所以正则按写的意思生效：`regex_match("^\d+\.\d{2}$", s)`。

## 条件表达式

```
if tier == "gold" then 0.20 else if tier == "silver" then 0.10 else 0.05
```

## 缺失的值

```
exists(user.discount)                       # 字段存在且不为 null 时为真
coalesce(user.nickname, user.name, "Guest") # 第一个不为 null 的值
```

## 函数

```
len(items) > 0
round(price * 0.85, 2)
starts_with(order_id, "ORD-")
```

完整列表见[内置函数](./builtin-functions)。

# 决策表

决策表是一种 `decision_table` 类型的步骤。每一行是一条规则：左边是输入条件，右边是输出值。和一串判断步骤相比，它通常更短，也更容易审阅。

```json
{
  "id": "discount",
  "name": "折扣",
  "type": "decision_table",
  "hit_policy": "first",
  "inputs": ["customer.tier", "order.amount"],
  "outputs": ["discount", "reason"],
  "rules": [
    { "when": ["gold", ">= 1000"], "then": [0.15, "金卡大单"] },
    { "when": [["silver", "bronze"], "100..999"], "then": [0.05, "= customer.tier + \" 会员\""] }
  ],
  "default": [0, "无折扣"],
  "next_step": "price"
}
```

- `inputs` 是表达式，每列一个。
- `outputs` 是变量名，后面的步骤用 `$discount`、`$reason` 读取。
- 行从上往下匹配。

同一张表写成 YAML：

```yaml
discount:
  id: discount
  name: 折扣
  type: decision_table
  inputs: [customer.tier, order.amount]
  outputs: [discount, reason]
  rules:
    - when: [gold, ">= 1000"]
      then: [0.15, "金卡大单"]
    - when: [[silver, bronze], "100..999"]
      then: [0.05, "= customer.tier + \" 会员\""]
  default: [0, "无折扣"]
  next_step: price
```

## 输入单元格

| 单元格                             | 什么时候匹配                       |
| ---------------------------------- | ---------------------------------- |
| `"*"`、`"-"`、`""` 或 `null`       | 总是匹配                           |
| `"gold"`、`42`、`true`             | 输入等于这个值                     |
| `["silver", "bronze"]`             | 输入是其中之一                     |
| `">= 1000"`、`"< 18"`、`"!= \"x\""` | 比较成立                          |
| `"in [...]"`、`"not in [...]"`     | 在或不在列表里                     |
| `"18..65"`                         | 数字在区间内，包含两端             |
| `"= <表达式>"`                     | 表达式为真（可以用任何字段）       |

普通字符串按文本比较。要和以运算符或 `in ` 开头的字符串比较，写成 `"== \"in stock\""`。

## 输出单元格

可以是字面量（`0.15`、`"Gold"`、`null`、数组或对象），也可以写 `"= <表达式>"` 来计算，比如 `"= order.amount * 0.1"`。

## 命中策略

| `hit_policy`   | 结果 |
| -------------- | ---- |
| `first`（默认） | 第一个匹配的行设置输出。没有匹配时用 `default`。没有 `default` 时执行失败，而不是带着缺失的值继续。 |
| `collect`      | 所有匹配的行，按顺序。每个输出变成数组；`aggregate` 为 `sum`、`count`、`min` 或 `max` 时变成一个值。 |

`collect` 没有匹配时，输出是 `[]`，`sum` 和 `count` 是 `0`，`min` 和 `max` 是 `null`。

把所有命中行的风险分加起来：

```json
{
  "id": "risk",
  "name": "风险分",
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

金额是 600 时，`$points` 是 30。

Studio 里也有表格视图，可以按表格编辑规则，见 [Studio](/zh/platform/studio)。

决策表步骤目前还不能编译成二进制 `.ordo` 格式。

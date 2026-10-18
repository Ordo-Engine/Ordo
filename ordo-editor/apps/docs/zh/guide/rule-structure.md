# 规则结构

一个规则文件（规则集）是 JSON 或 YAML，分两部分：`config` 和 `steps`。步骤组成一张图，从 `entry_step` 开始执行，到终止步骤结束。

```json
{
  "config": {
    "name": "loan-approval",
    "version": "1.0.0",
    "entry_step": "check_age"
  },
  "steps": {
    "check_age": { "id": "check_age", "name": "检查年龄", "type": "decision", "...": "..." }
  }
}
```

## config

| 字段           | 必填 | 说明                                                   |
| -------------- | ---- | ------------------------------------------------------ |
| `name`         | 是   | 规则集名称，调用方用它来执行规则                       |
| `version`      | 否   | 版本号，默认 `1.0.0`                                   |
| `description`  | 否   | 说明文字                                               |
| `entry_step`   | 是   | 第一个步骤的 ID                                        |
| `input_schema` | 否   | 声明输入字段，执行前先校验（见下文）                   |
| `timeout_ms`   | 否   | 执行超时，默认 `5000`                                  |
| `field_missing`| 否   | `lenient`（默认）或 `strict`，决定条件里缺字段时怎么处理 |

### 输入校验

没有声明时，缺字段会让它所在的比较为假。比如没传 `age`，`applicant.age < 18` 这条规则就被跳过了。声明之后，`ordo validate` 还会报出读取了未声明字段的规则，这通常是拼写错误。把需要的字段声明出来，错误的输入会被直接拒绝：

```json
"input_schema": [
  { "name": "applicant", "type": "object", "required": true, "fields": [
    { "name": "age", "type": "number", "required": true },
    { "name": "credit_score", "type": "number", "required": true }
  ]},
  { "name": "channel", "type": "string", "defaultValue": "online" },
  { "name": "items", "type": "array", "itemType": { "name": "item", "type": "object" } }
]
```

- `type`：`string`、`number`、`decimal`（精确小数，用于金额）、`boolean`、`array`、`object` 或 `any`。
- `required`：字段必须存在且不为 `null`。
- `defaultValue`：字段缺失或为 `null` 时使用。
- `fields` 和 `itemType` 用同样的方式校验嵌套对象和数组元素。

所有错误会一次性报出来。HTTP 返回 `400`，错误码 `INVALID_INPUT`，例如 `Invalid input: applicant.age: required field is missing`；gRPC 返回 `INVALID_ARGUMENT`。

## 步骤

每个步骤都有 `id`、`name` 和 `type`。`id` 要和 `steps` 里的键一致。`name` 必填，会显示在执行追踪里。

| `type`           | 作用                                                     |
| ---------------- | -------------------------------------------------------- |
| `decision`       | 按顺序检查条件，走第一个成立的分支                       |
| `decision_table` | 用表格的行匹配输入，设置变量。见[决策表](./decision-table) |
| `action`         | 执行动作（设置变量、写日志、调用外部服务），然后继续     |
| `sub_rule`       | 执行一段可复用的子图，再把结果带回来                     |
| `terminal`       | 结束执行，返回结果                                       |

### decision

```json
{
  "id": "check_amount",
  "name": "检查订单金额",
  "type": "decision",
  "branches": [
    { "condition": "order.amount > 1000", "next_step": "high_value" },
    { "condition": "order.amount > 100", "next_step": "medium_value" }
  ],
  "default_next": "low_value"
}
```

分支从上往下检查，第一个为真的条件生效。都不成立时走 `default_next`。条件是[表达式](./expression-syntax)。

### action

```json
{
  "id": "calc",
  "name": "计算总价",
  "type": "action",
  "actions": [
    { "action": "set_variable", "name": "subtotal", "value": "price * qty" },
    { "action": "set_variable", "name": "discount", "value": "if vip then 0.15 else 0" }
  ],
  "next_step": "done"
}
```

后面的步骤用 `$subtotal` 读取变量。可用的动作：

| `action`        | 字段                                           | 作用 |
| --------------- | ---------------------------------------------- | ---- |
| `set_variable`  | `name`、`value`                                | 计算 `value`，存为 `$name` |
| `log`           | `message`、`level`（`debug`、`info`、`warn`、`error`） | 写一行日志 |
| `metric`        | `name`、`value`、`tags`                        | 记录一个数值，ordo-server 会导出为 gauge `ordo_rule_<name>` |
| `external_call` | `service`、`method`、`params`、`result_variable`、`timeout_ms` | 调用宿主提供的能力（见下文） |
| `call_rule_set` | `ruleset_name`、`result_variable`，可选 `input_mapping` | 运行另一个规则集并保存结果（见下文） |

`value` 是表达式字符串。也接受 JSON AST 形式（`{"Binary": {...}}`），服务端保存的规则就是这种形式。

#### 外部调用

在 ordo-server 上，`external_call` 可以用这些内置能力：

| `service`            | `method`                                   | `params`                              |
| -------------------- | ------------------------------------------ | ------------------------------------- |
| `network.http`       | `GET`、`POST`、`PUT`、`PATCH`、`DELETE` 等（大写） | `url`，可选 `headers`、`json_body` |
| `metrics.prometheus` | `gauge`、`counter`                         | `name`、`value`，可选 `tags`          |
| `audit.logger`       | `rule_executed`                            | `rule_name`、`duration_us`、`result`  |

```json
{
  "action": "external_call",
  "service": "network.http",
  "method": "GET",
  "params": [["url", "\"https://api.example.com/score?id=\" + user.id"]],
  "result_variable": "score",
  "timeout_ms": 1000
}
```

响应存在 `$score` 下：`$score.payload.status`、`$score.payload.body`，响应体是 JSON 时还有 `$score.payload.json_body`。没有内置重试和降级，调用失败时在后面加一个判断步骤处理。自定义能力的写法见 [`examples/capability-demo`](https://github.com/Ordo-Engine/Ordo/tree/main/examples/capability-demo)。

#### 调用另一个规则集

`call_rule_set` 按名字运行另一个独立的规则集，这样几个规则集可以共用一份，比如同一套反欺诈检查：

```json
{
  "action": "call_rule_set",
  "ruleset_name": "fraud_check",
  "input_mapping": "{\"amount\": order.amount, \"country\": user.country}",
  "result_variable": "fraud"
}
```

结果存在 `$fraud` 下：`$fraud.code`、`$fraud.message` 和 `$fraud.output.<字段>`。不写 `input_mapping` 时，被调用的规则集拿到完整输入。两种情况下外部数据（`$data`）都会传过去。最多嵌套 10 层。

ordo-server 每次请求都在同一租户的规则集里按名字查找，所以改了 `fraud_check`，所有调用它的规则集都会用上新版本。在 Rust 里[嵌入](./embed)时，实现 `RuleSetResolver`，用 `executor.set_resolver(...)` 设置，或者用 `execute_with_resolver` 按次传入。命令行（`ordo exec`、`ordo test`）和编译后的 `.ordo` 文件暂不支持调用。想把一段图直接放进同一个文件，用 `sub_rule` 步骤。

### sub_rule

子图写在文件顶层的 `sub_rules` 里。`sub_rule` 步骤把输入绑定进去，再把变量带回来：

```json
{
  "config": { "name": "loan", "version": "1.0.0", "entry_step": "risk" },
  "sub_rules": {
    "risk_score": {
      "entry_step": "score",
      "steps": {
        "score": { "id": "score", "name": "打分", "type": "action",
          "actions": [{ "action": "set_variable", "name": "score", "value": "if age < 25 then 80 else 20" }],
          "next_step": "end" },
        "end": { "id": "end", "name": "结束", "type": "terminal", "result": { "code": "DONE" } }
      }
    }
  },
  "steps": {
    "risk": {
      "id": "risk", "name": "风险分", "type": "sub_rule",
      "ref_name": "risk_score",
      "bindings": [["age", "applicant.age"]],
      "outputs": [["risk", "score"]],
      "next_step": "done"
    },
    "done": { "id": "done", "name": "完成", "type": "terminal",
      "result": { "code": "OK", "output": [["risk", "$risk"]] } }
  }
}
```

`bindings` 是 `[子图字段, 父级表达式]` 对，`outputs` 是 `[父级变量, 子图变量]` 对。

### terminal

```json
{
  "id": "approved",
  "name": "通过",
  "type": "terminal",
  "result": {
    "code": "APPROVED",
    "message": "申请已通过",
    "output": [["limit", "applicant.income * 0.3"]],
    "data": { "tier": "standard" }
  }
}
```

| 字段      | 说明                                   |
| --------- | -------------------------------------- |
| `code`    | 结果码，调用方按它分支                 |
| `message` | 可选的文字说明                         |
| `output`  | `[名称, 表达式]` 对，在结束时计算      |
| `data`    | 固定值，合并进输出                     |

## 完整示例

```json
{
  "config": { "name": "loan-approval", "version": "1.0.0", "entry_step": "check_age" },
  "steps": {
    "check_age": {
      "id": "check_age", "name": "检查年龄", "type": "decision",
      "branches": [{ "condition": "applicant.age < 18", "next_step": "reject" }],
      "default_next": "check_income"
    },
    "check_income": {
      "id": "check_income", "name": "检查收入", "type": "decision",
      "branches": [{ "condition": "applicant.income >= 30000", "next_step": "approve" }],
      "default_next": "reject"
    },
    "approve": {
      "id": "approve", "name": "通过", "type": "terminal",
      "result": { "code": "APPROVED", "output": [["max_amount", "applicant.income * 2"]] }
    },
    "reject": {
      "id": "reject", "name": "拒绝", "type": "terminal",
      "result": { "code": "REJECTED" }
    }
  }
}
```

同样的内容写成 YAML 也一样能用。`ordo validate` 会编译每个步骤，并检查所有 `next_step` 指向的步骤都存在。

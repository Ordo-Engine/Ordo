# 规则结构

Ordo 规则使用**步骤流模型（Step Flow Model）**定义 —— 这一系列连接的步骤构成了一个决策树。

## 规则定义

一个规则（RuleSet）由两个主要部分组成：

```json
{
  "config": {
    "name": "rule-name",
    "version": "1.0.0",
    "description": "Optional description",
    "entry_step": "first_step"
  },
  "steps": {
    "first_step": { ... },
    "second_step": { ... }
  }
}
```

### 配置部分 (Config)

| 字段          | 类型   | 必填 | 描述                      |
| ------------- | ------ | ---- | ------------------------- |
| `name`        | string | 是   | 规则的唯一标识符          |
| `version`     | string | 是   | 语义化版本 (例如 "1.0.0") |
| `description` | string | 否   | 人类可读的描述            |
| `entry_step`  | string | 是   | 第一个要执行的步骤 ID     |

### 步骤部分 (Steps)

步骤 ID 到步骤定义的映射。每个步骤必须有一个唯一的 ID。

## 步骤类型

### 决策步骤 (Decision Step)

评估条件并分支到不同的步骤：

```json
{
  "id": "check_amount",
  "name": "Check Order Amount",
  "type": "decision",
  "branches": [
    {
      "condition": "order.amount > 1000",
      "next_step": "high_value"
    },
    {
      "condition": "order.amount > 100",
      "next_step": "medium_value"
    }
  ],
  "default_next": "low_value"
}
```

| 字段           | 类型   | 必填 | 描述                           |
| -------------- | ------ | ---- | ------------------------------ |
| `id`           | string | 是   | 唯一的步骤标识符               |
| `name`         | string | 否   | 人类可读的名称                 |
| `type`         | string | 是   | 必须是 `"decision"`            |
| `branches`     | array  | 是   | 基于条件的分支列表             |
| `default_next` | string | 是   | 如果没有分支匹配，则执行此步骤 |

### 终结步骤 (Terminal Step)

结束执行并返回结果：

```json
{
  "id": "approved",
  "name": "Application Approved",
  "type": "terminal",
  "result": {
    "code": "APPROVED",
    "message": "Your application has been approved",
    "data": {
      "credit_limit": 5000
    }
  }
}
```

| 字段     | 类型   | 必填 | 描述                |
| -------- | ------ | ---- | ------------------- |
| `id`     | string | 是   | 唯一的步骤标识符    |
| `name`   | string | 否   | 人类可读的名称      |
| `type`   | string | 是   | 必须是 `"terminal"` |
| `result` | object | 是   | 要返回的结果        |

### 结果对象 (Result Object)

| 字段      | 类型   | 必填 | 描述                                   |
| --------- | ------ | ---- | -------------------------------------- |
| `code`    | string | 是   | 结果代码 (例如 "APPROVED", "REJECTED") |
| `message` | string | 否   | 人类可读的消息                         |
| `data`    | object | 否   | 额外的输出数据                         |

## 分支条件

分支按顺序评估。第一个匹配的条件获胜。

```json
{
  "branches": [
    { "condition": "score >= 90", "next_step": "excellent" },
    { "condition": "score >= 70", "next_step": "good" },
    { "condition": "score >= 50", "next_step": "average" }
  ],
  "default_next": "poor"
}
```

## 完整示例

一个贷款审批规则：

```json
{
  "config": {
    "name": "loan-approval",
    "version": "1.0.0",
    "entry_step": "check_age"
  },
  "steps": {
    "check_age": {
      "id": "check_age",
      "name": "Age Verification",
      "type": "decision",
      "branches": [
        {
          "condition": "applicant.age < 18",
          "next_step": "reject_underage"
        }
      ],
      "default_next": "check_income"
    },
    "check_income": {
      "id": "check_income",
      "name": "Income Check",
      "type": "decision",
      "branches": [
        {
          "condition": "applicant.income >= 50000",
          "next_step": "approve_premium"
        },
        {
          "condition": "applicant.income >= 30000",
          "next_step": "approve_standard"
        }
      ],
      "default_next": "reject_income"
    },
    "approve_premium": {
      "id": "approve_premium",
      "type": "terminal",
      "result": {
        "code": "APPROVED",
        "message": "Premium loan approved",
        "data": { "tier": "premium", "max_amount": 100000 }
      }
    },
    "approve_standard": {
      "id": "approve_standard",
      "type": "terminal",
      "result": {
        "code": "APPROVED",
        "message": "Standard loan approved",
        "data": { "tier": "standard", "max_amount": 50000 }
      }
    },
    "reject_underage": {
      "id": "reject_underage",
      "type": "terminal",
      "result": {
        "code": "REJECTED",
        "message": "Applicant must be 18 or older"
      }
    },
    "reject_income": {
      "id": "reject_income",
      "type": "terminal",
      "result": {
        "code": "REJECTED",
        "message": "Income below minimum requirement"
      }
    }
  }
}
```

## 最佳实践

1.  **使用描述性的步骤 ID**：`check_vip_status` 优于 `step1`
2.  **添加步骤名称**：使追踪和调试更容易
3.  **按特异性排序分支**：最具体的条件放在最前面
4.  **始终有一个 default_next**：确保确定性执行
5.  **保持规则专注**：每个业务决策一个规则
6.  **版本化规则**：使用语义化版本跟踪变更

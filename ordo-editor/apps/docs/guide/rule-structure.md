# Rule Structure

Ordo rules are defined using a **Step Flow Model** - a series of connected steps that form a decision tree.

## Rule Definition

A rule (RuleSet) consists of two main parts:

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

### Config Section

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Unique identifier for the rule |
| `version` | string | Yes | Semantic version (e.g., "1.0.0") |
| `description` | string | No | Human-readable description |
| `entry_step` | string | Yes | ID of the first step to execute |

### Steps Section

A map of step IDs to step definitions. Each step must have a unique ID.

## Step Types

### Decision Step

Evaluates conditions and branches to different steps:

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

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique step identifier |
| `name` | string | No | Human-readable name |
| `type` | string | Yes | Must be `"decision"` |
| `branches` | array | Yes | List of condition-based branches |
| `default_next` | string | Yes | Step to execute if no branch matches |

### Terminal Step

Ends execution and returns a result:

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

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique step identifier |
| `name` | string | No | Human-readable name |
| `type` | string | Yes | Must be `"terminal"` |
| `result` | object | Yes | Result to return |

### Result Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `code` | string | Yes | Result code (e.g., "APPROVED", "REJECTED") |
| `message` | string | No | Human-readable message |
| `data` | object | No | Additional output data |

## Branch Conditions

Branches are evaluated in order. The first matching condition wins.

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

## Complete Example

A loan approval rule:

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

## Best Practices

1. **Use descriptive step IDs**: `check_vip_status` is better than `step1`
2. **Add step names**: Makes traces and debugging easier
3. **Order branches by specificity**: Most specific conditions first
4. **Always have a default_next**: Ensures deterministic execution
5. **Keep rules focused**: One rule per business decision
6. **Version your rules**: Use semantic versioning for tracking changes

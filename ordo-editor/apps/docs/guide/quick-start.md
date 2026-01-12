# Quick Start

Let's create and execute your first rule in under 5 minutes.

## Create a Rule

Create a simple discount rule that gives VIP users 20% off:

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H "Content-Type: application/json" \
  -d '{
    "config": {
      "name": "discount-check",
      "version": "1.0.0",
      "entry_step": "check_vip"
    },
    "steps": {
      "check_vip": {
        "id": "check_vip",
        "name": "Check VIP Status",
        "type": "decision",
        "branches": [
          {
            "condition": "user.vip == true",
            "next_step": "vip_discount"
          }
        ],
        "default_next": "normal_discount"
      },
      "vip_discount": {
        "id": "vip_discount",
        "name": "VIP Discount",
        "type": "terminal",
        "result": {
          "code": "VIP",
          "message": "20% discount applied",
          "discount": 0.20
        }
      },
      "normal_discount": {
        "id": "normal_discount",
        "name": "Normal Discount",
        "type": "terminal",
        "result": {
          "code": "NORMAL",
          "message": "5% discount applied",
          "discount": 0.05
        }
      }
    }
  }'
```

Response:

```json
{
  "status": "created",
  "name": "discount-check"
}
```

## Execute the Rule

### VIP User

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount-check \
  -H "Content-Type: application/json" \
  -d '{
    "input": {
      "user": {
        "id": "u123",
        "vip": true
      }
    }
  }'
```

Response:

```json
{
  "code": "VIP",
  "message": "20% discount applied",
  "output": {
    "discount": 0.2
  },
  "duration_us": 2
}
```

### Non-VIP User

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount-check \
  -H "Content-Type: application/json" \
  -d '{
    "input": {
      "user": {
        "id": "u456",
        "vip": false
      }
    }
  }'
```

Response:

```json
{
  "code": "NORMAL",
  "message": "5% discount applied",
  "output": {
    "discount": 0.05
  },
  "duration_us": 1
}
```

## Enable Tracing

Add `"trace": true` to see the execution path:

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount-check \
  -H "Content-Type: application/json" \
  -d '{
    "input": { "user": { "vip": true } },
    "trace": true
  }'
```

Response includes execution trace:

```json
{
  "code": "VIP",
  "message": "20% discount applied",
  "output": { "discount": 0.2 },
  "duration_us": 3,
  "trace": {
    "path": "check_vip -> vip_discount",
    "steps": [
      { "id": "check_vip", "name": "Check VIP Status", "duration_us": 1 },
      { "id": "vip_discount", "name": "VIP Discount", "duration_us": 0 }
    ]
  }
}
```

## List Rules

```bash
curl http://localhost:8080/api/v1/rulesets
```

## Delete a Rule

```bash
curl -X DELETE http://localhost:8080/api/v1/rulesets/discount-check
```

## Next Steps

- [Rule Structure](./rule-structure) - Learn about step types and branching
- [Expression Syntax](./expression-syntax) - Write complex conditions
- [HTTP API Reference](/api/http-api) - Full API documentation

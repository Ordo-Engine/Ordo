# HTTP REST API

Ordo provides a RESTful HTTP API for rule management and execution.

## Base URL

```
http://localhost:8080/api/v1
```

## Endpoints Overview

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/rulesets` | List all rules |
| POST | `/rulesets` | Create or update a rule |
| GET | `/rulesets/:name` | Get a rule by name |
| DELETE | `/rulesets/:name` | Delete a rule |
| POST | `/execute/:name` | Execute a rule |
| GET | `/rulesets/:name/versions` | List rule versions |
| POST | `/rulesets/:name/rollback` | Rollback to version |
| POST | `/eval` | Evaluate expression |
| GET | `/config/audit-sample-rate` | Get sample rate |
| PUT | `/config/audit-sample-rate` | Set sample rate |

---

## Rule Management

### List Rules

```http
GET /api/v1/rulesets
```

**Response:**

```json
[
  {
    "name": "discount-check",
    "version": "1.0.0",
    "description": "Check user discount eligibility"
  },
  {
    "name": "fraud-detection",
    "version": "2.1.0",
    "description": null
  }
]
```

### Get Rule

```http
GET /api/v1/rulesets/:name
```

**Response:**

```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "type": "decision",
      "branches": [...],
      "default_next": "normal"
    }
  }
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| 404 | Rule not found |

### Create or Update Rule

```http
POST /api/v1/rulesets
Content-Type: application/json
```

**Request Body:**

```json
{
  "config": {
    "name": "discount-check",
    "version": "1.0.0",
    "entry_step": "check_vip"
  },
  "steps": {
    "check_vip": {
      "id": "check_vip",
      "type": "decision",
      "branches": [
        { "condition": "user.vip == true", "next_step": "vip" }
      ],
      "default_next": "normal"
    },
    "vip": {
      "id": "vip",
      "type": "terminal",
      "result": { "code": "VIP", "message": "VIP discount" }
    },
    "normal": {
      "id": "normal",
      "type": "terminal",
      "result": { "code": "NORMAL", "message": "Normal discount" }
    }
  }
}
```

**Response (Created):**

```json
{
  "status": "created",
  "name": "discount-check"
}
```

**Response (Updated):**

```json
{
  "status": "updated",
  "name": "discount-check"
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| 400 | Validation error (invalid rule structure) |

### Delete Rule

```http
DELETE /api/v1/rulesets/:name
```

**Response:** `204 No Content`

**Errors:**

| Status | Description |
|--------|-------------|
| 404 | Rule not found |

---

## Rule Execution

### Execute Rule

```http
POST /api/v1/execute/:name
Content-Type: application/json
```

**Request Body:**

```json
{
  "input": {
    "user": {
      "id": "u123",
      "vip": true,
      "age": 25
    },
    "order": {
      "total": 150.00
    }
  },
  "trace": false
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `input` | object | Yes | Input data for rule evaluation |
| `trace` | boolean | No | Include execution trace (default: false) |

**Response:**

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "output": {
    "discount": 0.20
  },
  "duration_us": 2
}
```

**Response with Trace:**

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "output": { "discount": 0.20 },
  "duration_us": 3,
  "trace": {
    "path": "check_vip -> vip_discount",
    "steps": [
      { "id": "check_vip", "name": "Check VIP", "duration_us": 1 },
      { "id": "vip_discount", "name": "VIP Discount", "duration_us": 0 }
    ]
  }
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| 404 | Rule not found |
| 500 | Execution error |

---

## Version Management

### List Versions

```http
GET /api/v1/rulesets/:name/versions
```

**Response:**

```json
{
  "name": "discount-check",
  "current_version": "2.0.0",
  "versions": [
    {
      "seq": 3,
      "version": "1.5.0",
      "timestamp": "2024-01-08T10:00:00Z"
    },
    {
      "seq": 2,
      "version": "1.2.0",
      "timestamp": "2024-01-07T15:30:00Z"
    }
  ]
}
```

### Rollback

```http
POST /api/v1/rulesets/:name/rollback
Content-Type: application/json
```

**Request Body:**

```json
{
  "seq": 2
}
```

**Response:**

```json
{
  "status": "rolled_back",
  "name": "discount-check",
  "from_version": "2.0.0",
  "to_version": "1.2.0"
}
```

**Errors:**

| Status | Description |
|--------|-------------|
| 400 | Rollback not available (memory-only mode) |
| 404 | Rule or version not found |

---

## Expression Evaluation

### Evaluate Expression

Debug endpoint for testing expressions.

```http
POST /api/v1/eval
Content-Type: application/json
```

**Request Body:**

```json
{
  "expression": "user.age >= 18 && user.vip == true",
  "context": {
    "user": {
      "age": 25,
      "vip": true
    }
  }
}
```

**Response:**

```json
{
  "result": true,
  "parsed": "BinaryOp(And, ...)"
}
```

---

## Configuration

### Get Audit Sample Rate

```http
GET /api/v1/config/audit-sample-rate
```

**Response:**

```json
{
  "sample_rate": 10
}
```

### Set Audit Sample Rate

```http
PUT /api/v1/config/audit-sample-rate
Content-Type: application/json
```

**Request Body:**

```json
{
  "sample_rate": 50
}
```

**Response:**

```json
{
  "sample_rate": 50,
  "previous": 10
}
```

---

## Health & Metrics

### Health Check

```http
GET /health
```

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "storage": {
    "mode": "persistent",
    "rules_dir": "./rules",
    "rules_count": 12
  }
}
```

### Prometheus Metrics

```http
GET /metrics
```

**Response:**

```
# HELP ordo_info Ordo server info
# TYPE ordo_info gauge
ordo_info{version="0.1.0"} 1

# HELP ordo_uptime_seconds Server uptime
# TYPE ordo_uptime_seconds counter
ordo_uptime_seconds 3600

# HELP ordo_rules_total Number of loaded rules
# TYPE ordo_rules_total gauge
ordo_rules_total 12

# HELP ordo_executions_total Rule executions
# TYPE ordo_executions_total counter
ordo_executions_total{ruleset="discount-check",result="success"} 1000
```

---

## Error Responses

All errors follow this format:

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### Common Error Codes

| Status | Code | Description |
|--------|------|-------------|
| 400 | `BAD_REQUEST` | Invalid request body |
| 404 | `NOT_FOUND` | Resource not found |
| 500 | `INTERNAL_ERROR` | Server error |

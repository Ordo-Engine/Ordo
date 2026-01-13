# HTTP REST API

Ordo 提供了一个 RESTful HTTP API 用于规则管理和执行。

## 基础 URL

```
http://localhost:8080/api/v1
```

## 端点概览

| 方法   | 端点                        | 描述             |
| ------ | --------------------------- | ---------------- |
| GET    | `/rulesets`                 | 列出所有规则     |
| POST   | `/rulesets`                 | 创建或更新规则   |
| GET    | `/rulesets/:name`           | 根据名称获取规则 |
| DELETE | `/rulesets/:name`           | 删除规则         |
| POST   | `/execute/:name`            | 执行规则         |
| GET    | `/rulesets/:name/versions`  | 列出规则版本     |
| POST   | `/rulesets/:name/rollback`  | 回滚到版本       |
| POST   | `/eval`                     | 评估表达式       |
| GET    | `/config/audit-sample-rate` | 获取采样率       |
| PUT    | `/config/audit-sample-rate` | 设置采样率       |

---

## 规则管理

### 列出规则

```http
GET /api/v1/rulesets
```

**响应:**

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

### 获取规则

```http
GET /api/v1/rulesets/:name
```

**响应:**

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

**错误:**

| 状态码 | 描述       |
| ------ | ---------- |
| 404    | 规则未找到 |

### 创建或更新规则

```http
POST /api/v1/rulesets
Content-Type: application/json
```

**请求体:**

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
      "branches": [{ "condition": "user.vip == true", "next_step": "vip" }],
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

**响应 (创建成功):**

```json
{
  "status": "created",
  "name": "discount-check"
}
```

**响应 (更新成功):**

```json
{
  "status": "updated",
  "name": "discount-check"
}
```

**错误:**

| 状态码 | 描述                      |
| ------ | ------------------------- |
| 400    | 验证错误 (无效的规则结构) |

### 删除规则

```http
DELETE /api/v1/rulesets/:name
```

**响应:** `204 No Content`

**错误:**

| 状态码 | 描述       |
| ------ | ---------- |
| 404    | 规则未找到 |

---

## 规则执行

### 执行规则

```http
POST /api/v1/execute/:name
Content-Type: application/json
```

**请求体:**

```json
{
  "input": {
    "user": {
      "id": "u123",
      "vip": true,
      "age": 25
    },
    "order": {
      "total": 150.0
    }
  },
  "trace": false
}
```

| 字段    | 类型    | 必填 | 描述                       |
| ------- | ------- | ---- | -------------------------- |
| `input` | object  | 是   | 规则评估的输入数据         |
| `trace` | boolean | 否   | 包含执行追踪 (默认: false) |

**响应:**

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "output": {
    "discount": 0.2
  },
  "duration_us": 2
}
```

**包含追踪的响应:**

```json
{
  "code": "VIP",
  "message": "VIP discount applied",
  "output": { "discount": 0.2 },
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

**错误:**

| 状态码 | 描述       |
| ------ | ---------- |
| 404    | 规则未找到 |
| 500    | 执行错误   |

---

## 版本管理

### 列出版本

```http
GET /api/v1/rulesets/:name/versions
```

**响应:**

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

### 回滚

```http
POST /api/v1/rulesets/:name/rollback
Content-Type: application/json
```

**请求体:**

```json
{
  "seq": 2
}
```

**响应:**

```json
{
  "status": "rolled_back",
  "name": "discount-check",
  "from_version": "2.0.0",
  "to_version": "1.2.0"
}
```

**错误:**

| 状态码 | 描述                    |
| ------ | ----------------------- |
| 400    | 回滚不可用 (仅内存模式) |
| 404    | 规则或版本未找到        |

---

## 表达式评估

### 评估表达式

用于测试表达式的调试端点。

```http
POST /api/v1/eval
Content-Type: application/json
```

**请求体:**

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

**响应:**

```json
{
  "result": true,
  "parsed": "BinaryOp(And, ...)"
}
```

---

## 配置

### 获取审计采样率

```http
GET /api/v1/config/audit-sample-rate
```

**响应:**

```json
{
  "sample_rate": 10
}
```

### 设置审计采样率

```http
PUT /api/v1/config/audit-sample-rate
Content-Type: application/json
```

**请求体:**

```json
{
  "sample_rate": 50
}
```

**响应:**

```json
{
  "sample_rate": 50,
  "previous": 10
}
```

---

## 健康与指标

### 健康检查

```http
GET /health
```

**响应:**

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

### Prometheus 指标

```http
GET /metrics
```

**响应:**

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

## 错误响应

所有错误都遵循此格式：

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### 常见错误代码

| 状态码 | 代码             | 描述         |
| ------ | ---------------- | ------------ |
| 400    | `BAD_REQUEST`    | 无效的请求体 |
| 404    | `NOT_FOUND`      | 资源未找到   |
| 500    | `INTERNAL_ERROR` | 服务器错误   |

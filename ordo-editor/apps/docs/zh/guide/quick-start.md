# 快速入门

让我们在 5 分钟内创建并执行你的第一条规则。

## 创建规则

创建一个简单的折扣规则，为 VIP 用户提供 20% 的折扣：

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

响应：

```json
{
  "status": "created",
  "name": "discount-check"
}
```

## 执行规则

### VIP 用户

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

响应：

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

### 非 VIP 用户

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

响应：

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

## 启用追踪

添加 `"trace": true` 以查看执行路径：

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount-check \
  -H "Content-Type: application/json" \
  -d '{
    "input": { "user": { "vip": true } },
    "trace": true
  }'
```

响应包含执行追踪：

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

## 列出规则

```bash
curl http://localhost:8080/api/v1/rulesets
```

## 删除规则

```bash
curl -X DELETE http://localhost:8080/api/v1/rulesets/discount-check
```

## 下一步

- [规则结构](./rule-structure) - 了解步骤类型和分支
- [表达式语法](./expression-syntax) - 编写复杂条件
- [HTTP API 参考](../api/http-api) - 完整的 API 文档

# 版本管理

Ordo 自动维护规则的历史版本，支持回滚到先前的状态。

## 启用版本控制

版本控制需要启用持久化：

```bash
ordo-server --rules-dir ./rules --max-versions 10
```

| 标志             | 默认值 | 描述                     |
| ---------------- | ------ | ------------------------ |
| `--rules-dir`    | 无     | 规则存储目录             |
| `--max-versions` | 10     | 每个规则保留的最大版本数 |

## 工作原理

1.  当规则被**更新**时，当前版本将保存到 `.versions/`
2.  超过 `--max-versions` 的旧版本将被自动删除
3.  可以通过 API 列出和恢复版本

## 版本存储

```
rules/
├── discount-check.json          # 当前版本
└── .versions/
    └── discount-check/
        ├── v1_1704700000.json   # seq=1, timestamp
        ├── v2_1704800000.json   # seq=2, timestamp
        └── v3_1704900000.json   # seq=3, timestamp
```

## 列出版本

```bash
curl http://localhost:8080/api/v1/rulesets/discount-check/versions
```

响应：

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
    },
    {
      "seq": 1,
      "version": "1.0.0",
      "timestamp": "2024-01-06T09:00:00Z"
    }
  ]
}
```

## 回滚

将规则恢复到以前的版本：

```bash
curl -X POST http://localhost:8080/api/v1/rulesets/discount-check/rollback \
  -H "Content-Type: application/json" \
  -d '{"seq": 2}'
```

响应：

```json
{
  "status": "rolled_back",
  "name": "discount-check",
  "from_version": "2.0.0",
  "to_version": "1.2.0"
}
```

::: warning
回滚会创建一个新的版本条目。回滚前的版本将保留在历史记录中。
:::

## 版本清理

超过 `--max-versions` 的版本将被自动删除（最旧的先删除）：

```bash
# 仅保留最后 5 个版本
ordo-server --rules-dir ./rules --max-versions 5
```

## 审计跟踪

当启用 [审计日志](./audit-logging) 时，版本更改将被记录：

```json
{"event":"rule_updated","rule_name":"discount-check","from_version":"1.0.0","to_version":"2.0.0"}
{"event":"rule_rollback","rule_name":"discount-check","from_version":"2.0.0","to_version":"1.0.0","seq":1}
```

## 最佳实践

1.  **设置合适的最大版本数**：平衡存储与回滚深度
2.  **使用语义化版本**：`1.0.0` → `1.0.1` → `1.1.0` → `2.0.0`
3.  **记录更改**：使用有意义的版本号
4.  **回滚前测试**：验证目标版本是否正常工作
5.  **监控版本增长**：在生产环境中关注磁盘使用情况

## 仅内存模式

如果没有 `--rules-dir`，版本控制不可用：

```bash
curl http://localhost:8080/api/v1/rulesets/my-rule/versions
```

```json
{
  "name": "my-rule",
  "current_version": "1.0.0",
  "versions": []
}
```

回滚会返回错误：

```json
{
  "error": "Version rollback not available in memory-only mode"
}
```

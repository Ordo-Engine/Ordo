# 审计日志

Ordo 提供结构化的审计日志，用于跟踪规则变更、执行和系统事件。

## 启用审计日志

```bash
ordo-server --audit-dir ./audit --audit-sample-rate 10
```

| 标志                  | 默认值 | 描述                |
| --------------------- | ------ | ------------------- |
| `--audit-dir`         | 无     | 审计日志文件目录    |
| `--audit-sample-rate` | 10     | 执行采样率 (0-100%) |

## 日志输出

审计事件将写入：

1.  **stdout** - 始终（通过 tracing）
2.  **文件** - 当指定了 `--audit-dir` 时

## 文件格式

日志以 JSON Lines 格式写入，每日轮换：

```
audit/
├── audit-2024-01-08.jsonl
├── audit-2024-01-09.jsonl
└── audit-2024-01-10.jsonl
```

每一行都是一个完整的 JSON 对象：

```json
{"timestamp":"2024-01-08T10:00:00.123Z","level":"INFO","event":"server_started","version":"0.1.0","rules_count":12}
{"timestamp":"2024-01-08T10:00:01.456Z","level":"INFO","event":"rule_created","rule_name":"payment-check","version":"1.0.0","source_ip":"127.0.0.1"}
{"timestamp":"2024-01-08T10:00:02.789Z","level":"INFO","event":"rule_executed","rule_name":"payment-check","duration_us":1500,"result":"success"}
```

## 事件类型

### 系统事件

| 事件             | 描述       | 字段                     |
| ---------------- | ---------- | ------------------------ |
| `server_started` | 服务器启动 | `version`, `rules_count` |
| `server_stopped` | 服务器关闭 | `uptime_seconds`         |

### 规则变更事件

| 事件            | 描述       | 字段                                                          |
| --------------- | ---------- | ------------------------------------------------------------- |
| `rule_created`  | 创建新规则 | `rule_name`, `version`, `source_ip`                           |
| `rule_updated`  | 更新规则   | `rule_name`, `from_version`, `to_version`, `source_ip`        |
| `rule_deleted`  | 删除规则   | `rule_name`, `source_ip`                                      |
| `rule_rollback` | 版本回滚   | `rule_name`, `from_version`, `to_version`, `seq`, `source_ip` |

### 执行事件

| 事件            | 描述            | 字段                                              |
| --------------- | --------------- | ------------------------------------------------- |
| `rule_executed` | 规则执行 (采样) | `rule_name`, `duration_us`, `result`, `source_ip` |

### 配置事件

| 事件                  | 描述       | 字段                                |
| --------------------- | ---------- | ----------------------------------- |
| `sample_rate_changed` | 采样率更新 | `from_rate`, `to_rate`, `source_ip` |

## 执行采样

为了避免在高吞吐量场景中日志过多，执行事件会被采样：

- `--audit-sample-rate 10` → 记录约 10% 的执行
- `--audit-sample-rate 100` → 记录所有执行
- `--audit-sample-rate 0` → 禁用执行日志

### 动态采样率

在运行时无需重启即可更新采样率：

```bash
# 获取当前采样率
curl http://localhost:8080/api/v1/config/audit-sample-rate
```

```json
{ "sample_rate": 10 }
```

```bash
# 更新为 50%
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sample_rate": 50}'
```

```json
{ "sample_rate": 50, "previous": 10 }
```

## 用例

### 合规审计

追踪谁在何时更改了什么：

```bash
grep "rule_updated\|rule_created\|rule_deleted" audit/audit-2024-01-08.jsonl
```

### 性能分析

分析执行时间：

```bash
grep "rule_executed" audit/audit-2024-01-08.jsonl | \
  jq -r '.duration_us' | \
  awk '{sum+=$1; count++} END {print "avg:", sum/count, "us"}'
```

### 错误调查

查找失败的执行：

```bash
grep '"result":"error"' audit/audit-2024-01-08.jsonl
```

### 安全监控

追踪访问模式：

```bash
grep "source_ip" audit/audit-2024-01-08.jsonl | \
  jq -r '.source_ip' | sort | uniq -c | sort -rn
```

## 日志轮换

日志每天自动轮换。如需额外的轮换（基于大小、压缩），请使用外部工具：

```bash
# logrotate 配置示例
/path/to/audit/*.jsonl {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
}
```

## 最佳实践

1.  **设置合适的采样率**：从 10% 开始，根据需要增加
2.  **监控磁盘使用**：审计日志可能增长很快
3.  **实施日志轮换**：使用 logrotate 或类似工具
4.  **归档旧日志**：移动到冷存储以满足合规性要求
5.  **保护日志访问**：审计日志可能包含敏感信息
6.  **使用 jq 解析**：JSON Lines 格式非常适合用 jq 处理

## 与日志聚合器集成

### Fluentd

```yaml
<source>
@type tail
path /var/log/ordo/audit/*.jsonl
pos_file /var/log/fluentd/ordo-audit.pos
tag ordo.audit
<parse>
@type json
</parse>
</source>
```

### Filebeat

```yaml
filebeat.inputs:
  - type: log
    paths:
      - /var/log/ordo/audit/*.jsonl
    json.keys_under_root: true
    json.add_error_key: true
```

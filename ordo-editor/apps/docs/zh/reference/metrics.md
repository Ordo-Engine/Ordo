# 指标

`ordo-server` 在 HTTP 端口的 `GET /metrics` 提供 Prometheus 指标。它总是开启，也从不需要 API token。

```yaml
# prometheus.yml
scrape_configs:
  - job_name: ordo
    static_configs:
      - targets: ['ordo:8080']
```

## 执行

| 指标 | 类型 | 标签 |
| --- | --- | --- |
| `ordo_executions_total` | counter | `ruleset`、`result`（`success`、`error`） |
| `ordo_execution_duration_seconds` | histogram | `ruleset` |
| `ordo_terminal_results_total` | counter | `ruleset`、`result_code` |
| `ordo_active_executions` | gauge | |
| `ordo_batch_executions_total` | counter | `ruleset` |
| `ordo_batch_size` | histogram | `ruleset` |
| `ordo_batch_duration_seconds` | histogram | `ruleset` |
| `ordo_batch_results_total` | counter | `ruleset`、`result` |
| `ordo_evals_total` | counter | `result` |
| `ordo_eval_duration_seconds` | histogram | |

开启多租户时，会多出 `ordo_tenant_executions_total`（`tenant_id`、`ruleset`、`result`）、`ordo_tenant_execution_duration_seconds`、`ordo_tenant_rules_total` 和 `ordo_tenant_rate_limited_total`。

## 服务和存储

| 指标 | 类型 | 标签 |
| --- | --- | --- |
| `ordo_info` | gauge | `version` |
| `ordo_uptime_seconds` | gauge | |
| `ordo_rules_total` | gauge | |
| `ordo_store_operations_total` | counter | `operation` |
| `ordo_hot_reloads_total` | counter | `kind`、`result` |
| `ordo_last_reload_timestamp_seconds` | gauge | |
| `ordo_wal_size_bytes`、`ordo_wal_segments_total`、`ordo_wal_pending_entries` | gauge | |
| `ordo_wal_writes_total` | counter | `op`、`state` |
| `ordo_wal_replays_total` | counter | `op`、`result` |
| `ordo_wal_rotations_total` | gauge | |
| `ordo_sync_events_published_total`、`ordo_sync_events_applied_total` | counter | `event_type` |
| `ordo_sync_events_failed_total` | counter | `event_type`、`stage` |
| `ordo_last_sync_timestamp_seconds` | gauge | |

## 规则里的指标

规则中的 `metric` 动作会导出为 gauge `ordo_rule_<name>`，tags 作为标签：

```json
{ "action": "metric", "name": "order_amount", "value": "order.amount", "tags": [["tier", "gold"]] }
```

```text
ordo_rule_order_amount{tier="gold"} 42
```

## 常用查询

```text
# 每秒执行数，按规则集
sum by (ruleset) (rate(ordo_executions_total[5m]))

# 错误比例
sum(rate(ordo_executions_total{result="error"}[5m])) / sum(rate(ordo_executions_total[5m]))

# p99 延迟
histogram_quantile(0.99, sum by (le, ruleset) (rate(ordo_execution_duration_seconds_bucket[5m])))

# 各结果码的占比
sum by (result_code) (rate(ordo_terminal_results_total{ruleset="discount"}[1h]))
```

第一批告警可以设在错误比例和 `up{job="ordo"} == 0` 上。

## 追踪

设置 `--otlp-endpoint`（比如 `http://otel-collector:4318`），通过 OTLP HTTP 导出 OpenTelemetry 追踪。可以接 OpenTelemetry Collector、Jaeger、Tempo 等 OTLP 后端。`--service-name` 设置服务名。

# Metrics

`ordo-server` serves Prometheus metrics at `GET /metrics` on the HTTP port. It is always on, and never needs the API token.

```yaml
# prometheus.yml
scrape_configs:
  - job_name: ordo
    static_configs:
      - targets: ['ordo:8080']
```

## Executions

| Metric | Type | Labels |
| --- | --- | --- |
| `ordo_executions_total` | counter | `ruleset`, `result` (`success`, `error`) |
| `ordo_execution_duration_seconds` | histogram | `ruleset` |
| `ordo_terminal_results_total` | counter | `ruleset`, `result_code` |
| `ordo_active_executions` | gauge | |
| `ordo_batch_executions_total` | counter | `ruleset` |
| `ordo_batch_size` | histogram | `ruleset` |
| `ordo_batch_duration_seconds` | histogram | `ruleset` |
| `ordo_batch_results_total` | counter | `ruleset`, `result` |
| `ordo_evals_total` | counter | `result` |
| `ordo_eval_duration_seconds` | histogram | |

With multi-tenancy, `ordo_tenant_executions_total` (`tenant_id`, `ruleset`, `result`), `ordo_tenant_execution_duration_seconds`, `ordo_tenant_rules_total` and `ordo_tenant_rate_limited_total` are added.

## Server and storage

| Metric | Type | Labels |
| --- | --- | --- |
| `ordo_info` | gauge | `version` |
| `ordo_uptime_seconds` | gauge | |
| `ordo_rules_total` | gauge | |
| `ordo_store_operations_total` | counter | `operation` |
| `ordo_hot_reloads_total` | counter | `kind`, `result` |
| `ordo_last_reload_timestamp_seconds` | gauge | |
| `ordo_wal_size_bytes`, `ordo_wal_segments_total`, `ordo_wal_pending_entries` | gauge | |
| `ordo_wal_writes_total` | counter | `op`, `state` |
| `ordo_wal_replays_total` | counter | `op`, `result` |
| `ordo_wal_rotations_total` | gauge | |
| `ordo_sync_events_published_total`, `ordo_sync_events_applied_total` | counter | `event_type` |
| `ordo_sync_events_failed_total` | counter | `event_type`, `stage` |
| `ordo_last_sync_timestamp_seconds` | gauge | |

## Metrics from rules

A `metric` action in a rule is exported as the gauge `ordo_rule_<name>`, with its tags as labels:

```json
{ "action": "metric", "name": "order_amount", "value": "order.amount", "tags": [["tier", "gold"]] }
```

```text
ordo_rule_order_amount{tier="gold"} 42
```

## Useful queries

```text
# executions per second, by ruleset
sum by (ruleset) (rate(ordo_executions_total[5m]))

# error ratio
sum(rate(ordo_executions_total{result="error"}[5m])) / sum(rate(ordo_executions_total[5m]))

# p99 latency
histogram_quantile(0.99, sum by (le, ruleset) (rate(ordo_execution_duration_seconds_bucket[5m])))

# share of each result code
sum by (result_code) (rate(ordo_terminal_results_total{ruleset="discount"}[1h]))
```

A reasonable first alert is on the error ratio, and on `up{job="ordo"} == 0`.

## Traces

Set `--otlp-endpoint` (for example `http://otel-collector:4318`) to export OpenTelemetry traces over OTLP HTTP. Works with the OpenTelemetry Collector, Jaeger, Tempo and other OTLP backends. `--service-name` sets the service name.

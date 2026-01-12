# Metrics

Ordo exposes Prometheus-compatible metrics for monitoring and alerting.

## Endpoint

```
GET /metrics
```

## Available Metrics

### Server Info

```
# HELP ordo_info Ordo server information
# TYPE ordo_info gauge
ordo_info{version="0.1.0"} 1
```

### Uptime

```
# HELP ordo_uptime_seconds Server uptime in seconds
# TYPE ordo_uptime_seconds counter
ordo_uptime_seconds 3600
```

### Rules Count

```
# HELP ordo_rules_total Total number of loaded rules
# TYPE ordo_rules_total gauge
ordo_rules_total 12
```

### Execution Metrics

```
# HELP ordo_executions_total Total rule executions
# TYPE ordo_executions_total counter
ordo_executions_total{ruleset="discount-check",result="success"} 1000
ordo_executions_total{ruleset="discount-check",result="error"} 5
ordo_executions_total{ruleset="fraud-check",result="success"} 500

# HELP ordo_execution_duration_seconds Rule execution duration
# TYPE ordo_execution_duration_seconds histogram
ordo_execution_duration_seconds_bucket{ruleset="discount-check",le="0.0001"} 900
ordo_execution_duration_seconds_bucket{ruleset="discount-check",le="0.0005"} 990
ordo_execution_duration_seconds_bucket{ruleset="discount-check",le="0.001"} 1000
ordo_execution_duration_seconds_bucket{ruleset="discount-check",le="+Inf"} 1000
ordo_execution_duration_seconds_sum{ruleset="discount-check"} 1.5
ordo_execution_duration_seconds_count{ruleset="discount-check"} 1000
```

### Expression Evaluation

```
# HELP ordo_eval_total Total expression evaluations
# TYPE ordo_eval_total counter
ordo_eval_total{result="success"} 100
ordo_eval_total{result="error"} 2

# HELP ordo_eval_duration_seconds Expression evaluation duration
# TYPE ordo_eval_duration_seconds histogram
ordo_eval_duration_seconds_bucket{le="0.00001"} 80
ordo_eval_duration_seconds_bucket{le="0.0001"} 98
ordo_eval_duration_seconds_bucket{le="+Inf"} 100
```

## Prometheus Configuration

### scrape_configs

```yaml
scrape_configs:
  - job_name: 'ordo'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

### Service Discovery (Kubernetes)

```yaml
scrape_configs:
  - job_name: 'ordo'
    kubernetes_sd_configs:
      - role: pod
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        regex: ordo
        action: keep
```

## Grafana Dashboard

### Example Queries

**Request Rate:**
```promql
rate(ordo_executions_total[5m])
```

**Error Rate:**
```promql
rate(ordo_executions_total{result="error"}[5m]) / rate(ordo_executions_total[5m])
```

**P99 Latency:**
```promql
histogram_quantile(0.99, rate(ordo_execution_duration_seconds_bucket[5m]))
```

**Rules Count:**
```promql
ordo_rules_total
```

**Uptime:**
```promql
ordo_uptime_seconds
```

### Dashboard JSON

```json
{
  "title": "Ordo Dashboard",
  "panels": [
    {
      "title": "Request Rate",
      "type": "graph",
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total[5m])) by (ruleset)",
          "legendFormat": "{{ruleset}}"
        }
      ]
    },
    {
      "title": "Latency (P99)",
      "type": "graph",
      "targets": [
        {
          "expr": "histogram_quantile(0.99, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le, ruleset))",
          "legendFormat": "{{ruleset}}"
        }
      ]
    },
    {
      "title": "Error Rate",
      "type": "graph",
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total{result=\"error\"}[5m])) by (ruleset)",
          "legendFormat": "{{ruleset}}"
        }
      ]
    }
  ]
}
```

## Alerting Rules

### Prometheus Alerts

```yaml
groups:
  - name: ordo
    rules:
      - alert: OrdoHighErrorRate
        expr: |
          sum(rate(ordo_executions_total{result="error"}[5m])) 
          / sum(rate(ordo_executions_total[5m])) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate in Ordo"
          description: "Error rate is above 5%"

      - alert: OrdoHighLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          ) > 0.01
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High latency in Ordo"
          description: "P99 latency is above 10ms"

      - alert: OrdoDown
        expr: up{job="ordo"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Ordo server is down"
```

## Best Practices

1. **Set appropriate scrape interval**: 15-30 seconds is typical
2. **Use recording rules**: Pre-compute common queries
3. **Set up alerts**: Monitor error rate and latency
4. **Retain metrics**: Keep at least 15 days for trend analysis
5. **Label cardinality**: Avoid high-cardinality labels

## Metric Labels

| Metric | Labels |
|--------|--------|
| `ordo_info` | `version` |
| `ordo_executions_total` | `ruleset`, `result` |
| `ordo_execution_duration_seconds` | `ruleset` |
| `ordo_eval_total` | `result` |
| `ordo_eval_duration_seconds` | (none) |

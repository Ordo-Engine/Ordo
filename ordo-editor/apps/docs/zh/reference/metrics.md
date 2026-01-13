# 指标

Ordo 暴露了兼容 Prometheus 的指标，用于监控和告警。

## 端点

```
GET /metrics
```

## 可用指标

### 服务器信息

```
# HELP ordo_info Ordo server information
# TYPE ordo_info gauge
ordo_info{version="0.1.0"} 1
```

### 运行时间

```
# HELP ordo_uptime_seconds Server uptime in seconds
# TYPE ordo_uptime_seconds counter
ordo_uptime_seconds 3600
```

### 规则计数

```
# HELP ordo_rules_total Total number of loaded rules
# TYPE ordo_rules_total gauge
ordo_rules_total 12
```

### 执行指标

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

### 表达式评估

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

## Prometheus 配置

### scrape_configs

```yaml
scrape_configs:
  - job_name: 'ordo'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
    scrape_interval: 15s
```

### 服务发现 (Kubernetes)

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

## Grafana 仪表板

### 示例查询

**请求速率:**

```
rate(ordo_executions_total[5m])
```

**错误率:**

```
rate(ordo_executions_total{result="error"}[5m]) / rate(ordo_executions_total[5m])
```

**P99 延迟:**

```
histogram_quantile(0.99, rate(ordo_execution_duration_seconds_bucket[5m]))
```

**规则计数:**

```
ordo_rules_total
```

**运行时间:**

```
ordo_uptime_seconds
```

### 仪表板 JSON

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

## 告警规则

### Prometheus 告警

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
          summary: 'High error rate in Ordo'
          description: 'Error rate is above 5%'

      - alert: OrdoHighLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          ) > 0.01
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'High latency in Ordo'
          description: 'P99 latency is above 10ms'

      - alert: OrdoDown
        expr: up{job="ordo"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: 'Ordo server is down'
```

## 最佳实践

1.  **设置合适的抓取间隔**：通常为 15-30 秒
2.  **使用记录规则**：预计算常见查询
3.  **设置告警**：监控错误率和延迟
4.  **保留指标**：至少保留 15 天以进行趋势分析
5.  **标签基数**：避免高基数标签

## 指标标签

| 指标                              | 标签                |
| --------------------------------- | ------------------- |
| `ordo_info`                       | `version`           |
| `ordo_executions_total`           | `ruleset`, `result` |
| `ordo_execution_duration_seconds` | `ruleset`           |
| `ordo_eval_total`                 | `result`            |
| `ordo_eval_duration_seconds`      | (无)                |

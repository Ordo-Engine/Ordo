# Metrics & Monitoring

Ordo exposes Prometheus-compatible metrics for comprehensive monitoring, alerting, and performance analysis.

## Quick Start

```bash
# Start Ordo with metrics enabled (default)
ordo-server --metrics-enabled

# Access metrics endpoint
curl http://localhost:8080/metrics
```

## Endpoint

```
GET /metrics
```

Returns metrics in Prometheus text format.

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

```
rate(ordo_executions_total[5m])
```

**Error Rate:**

```
rate(ordo_executions_total{result="error"}[5m]) / rate(ordo_executions_total[5m])
```

**P99 Latency:**

```
histogram_quantile(0.99, rate(ordo_execution_duration_seconds_bucket[5m]))
```

**Rules Count:**

```
ordo_rules_total
```

**Uptime:**

```
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

## Monitoring Stack Deployment

### Docker Compose (Quick Setup)

Create a complete monitoring stack with Prometheus, Grafana, and Alertmanager:

```yaml
# docker-compose.monitoring.yml
version: '3.8'

services:
  ordo:
    image: ghcr.io/pama-lee/ordo:latest
    ports:
      - '8080:8080'
    environment:
      - ORDO_METRICS_ENABLED=true
    volumes:
      - ./rules:/data/rules

  prometheus:
    image: prom/prometheus:latest
    ports:
      - '9090:9090'
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - ./alerts.yml:/etc/prometheus/alerts.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=15d'

  grafana:
    image: grafana/grafana:latest
    ports:
      - '3000:3000'
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources

  alertmanager:
    image: prom/alertmanager:latest
    ports:
      - '9093:9093'
    volumes:
      - ./alertmanager.yml:/etc/alertmanager/alertmanager.yml

volumes:
  prometheus_data:
  grafana_data:
```

### Prometheus Configuration File

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']

rule_files:
  - '/etc/prometheus/alerts.yml'

scrape_configs:
  - job_name: 'ordo'
    static_configs:
      - targets: ['ordo:8080']
    metrics_path: /metrics
    scrape_interval: 10s
    scrape_timeout: 5s
```

### Kubernetes Deployment

#### ServiceMonitor (Prometheus Operator)

```yaml
# servicemonitor.yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: ordo
  labels:
    app: ordo
    release: prometheus
spec:
  selector:
    matchLabels:
      app: ordo
  endpoints:
    - port: http
      path: /metrics
      interval: 15s
      scrapeTimeout: 10s
  namespaceSelector:
    matchNames:
      - default
```

#### PodMonitor (Alternative)

```yaml
# podmonitor.yaml
apiVersion: monitoring.coreos.com/v1
kind: PodMonitor
metadata:
  name: ordo
spec:
  selector:
    matchLabels:
      app: ordo
  podMetricsEndpoints:
    - port: http
      path: /metrics
      interval: 15s
```

### HashiCorp Nomad + Consul

```hcl
# ordo.nomad
job "ordo" {
  datacenters = ["dc1"]

  group "ordo" {
    network {
      port "http" { to = 8080 }
    }

    service {
      name = "ordo"
      port = "http"

      tags = [
        "prometheus.io/scrape=true",
        "prometheus.io/port=8080",
        "prometheus.io/path=/metrics"
      ]

      check {
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "2s"
      }
    }

    task "ordo" {
      driver = "docker"

      config {
        image = "ghcr.io/pama-lee/ordo:latest"
        ports = ["http"]
      }
    }
  }
}
```

## Performance Monitoring

### Key Performance Metrics

Based on the [benchmark results](/en/reference/benchmarks), monitor these critical thresholds:

| Metric                 | Expected Value | Alert Threshold |
| ---------------------- | -------------- | --------------- |
| Simple Rule Execution  | ~350 ns        | > 1 µs          |
| Complex Rule Execution | ~2 µs          | > 10 µs         |
| Throughput             | ~2.8M ops/sec  | < 1M ops/sec    |
| Initialization Time    | ~50 ns         | > 1 µs          |
| Error Rate             | < 0.1%         | > 1%            |
| P99 Latency            | < 5 ms         | > 50 ms         |

### Performance Dashboard Queries

**Execution Throughput (ops/sec):**

```promql
sum(rate(ordo_executions_total[1m]))
```

**Average Execution Time:**

```promql
rate(ordo_execution_duration_seconds_sum[5m])
/ rate(ordo_execution_duration_seconds_count[5m])
```

**P50/P95/P99 Latency:**

```promql
# P50
histogram_quantile(0.50, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))

# P95
histogram_quantile(0.95, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))

# P99
histogram_quantile(0.99, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))
```

**Execution Success Rate:**

```promql
sum(rate(ordo_executions_total{result="success"}[5m]))
/ sum(rate(ordo_executions_total[5m])) * 100
```

## Complete Grafana Dashboard

```json
{
  "title": "Ordo Rule Engine Dashboard",
  "uid": "ordo-main",
  "tags": ["ordo", "rule-engine"],
  "timezone": "browser",
  "panels": [
    {
      "title": "Throughput (ops/sec)",
      "type": "stat",
      "gridPos": { "x": 0, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total[1m]))",
          "legendFormat": "Total"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "unit": "ops",
          "thresholds": {
            "mode": "absolute",
            "steps": [
              { "value": 0, "color": "red" },
              { "value": 100000, "color": "yellow" },
              { "value": 1000000, "color": "green" }
            ]
          }
        }
      }
    },
    {
      "title": "Error Rate",
      "type": "stat",
      "gridPos": { "x": 6, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total{result=\"error\"}[5m])) / sum(rate(ordo_executions_total[5m])) * 100",
          "legendFormat": "Error %"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "unit": "percent",
          "thresholds": {
            "mode": "absolute",
            "steps": [
              { "value": 0, "color": "green" },
              { "value": 1, "color": "yellow" },
              { "value": 5, "color": "red" }
            ]
          }
        }
      }
    },
    {
      "title": "P99 Latency",
      "type": "stat",
      "gridPos": { "x": 12, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "histogram_quantile(0.99, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))",
          "legendFormat": "P99"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "unit": "s",
          "thresholds": {
            "mode": "absolute",
            "steps": [
              { "value": 0, "color": "green" },
              { "value": 0.01, "color": "yellow" },
              { "value": 0.05, "color": "red" }
            ]
          }
        }
      }
    },
    {
      "title": "Active Rules",
      "type": "stat",
      "gridPos": { "x": 18, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "ordo_rules_total",
          "legendFormat": "Rules"
        }
      ]
    },
    {
      "title": "Request Rate by Ruleset",
      "type": "timeseries",
      "gridPos": { "x": 0, "y": 4, "w": 12, "h": 8 },
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total[5m])) by (ruleset)",
          "legendFormat": "{{ruleset}}"
        }
      ]
    },
    {
      "title": "Latency Distribution",
      "type": "timeseries",
      "gridPos": { "x": 12, "y": 4, "w": 12, "h": 8 },
      "targets": [
        {
          "expr": "histogram_quantile(0.50, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))",
          "legendFormat": "P50"
        },
        {
          "expr": "histogram_quantile(0.95, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))",
          "legendFormat": "P95"
        },
        {
          "expr": "histogram_quantile(0.99, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))",
          "legendFormat": "P99"
        }
      ]
    },
    {
      "title": "Terminal Results Distribution",
      "type": "piechart",
      "gridPos": { "x": 0, "y": 12, "w": 8, "h": 8 },
      "targets": [
        {
          "expr": "sum(ordo_terminal_results_total) by (code)",
          "legendFormat": "{{code}}"
        }
      ]
    },
    {
      "title": "Errors by Ruleset",
      "type": "timeseries",
      "gridPos": { "x": 8, "y": 12, "w": 16, "h": 8 },
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

## Complete Alert Rules

```yaml
# alerts.yml
groups:
  - name: ordo-availability
    rules:
      - alert: OrdoDown
        expr: up{job="ordo"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: 'Ordo server is down'
          description: 'Ordo instance {{ $labels.instance }} has been down for more than 1 minute.'

      - alert: OrdoHighRestartRate
        expr: changes(process_start_time_seconds{job="ordo"}[1h]) > 3
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'Ordo is restarting frequently'
          description: 'Ordo has restarted {{ $value }} times in the last hour.'

  - name: ordo-performance
    rules:
      - alert: OrdoHighErrorRate
        expr: |
          sum(rate(ordo_executions_total{result="error"}[5m])) 
          / sum(rate(ordo_executions_total[5m])) > 0.01
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'High error rate in Ordo'
          description: 'Error rate is {{ $value | humanizePercentage }} (threshold: 1%)'

      - alert: OrdoCriticalErrorRate
        expr: |
          sum(rate(ordo_executions_total{result="error"}[5m])) 
          / sum(rate(ordo_executions_total[5m])) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: 'Critical error rate in Ordo'
          description: 'Error rate is {{ $value | humanizePercentage }} (threshold: 5%)'

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
          description: 'P99 latency is {{ $value | humanizeDuration }} (threshold: 10ms)'

      - alert: OrdoCriticalLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          ) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: 'Critical latency in Ordo'
          description: 'P99 latency is {{ $value | humanizeDuration }} (threshold: 50ms)'

      - alert: OrdoLowThroughput
        expr: sum(rate(ordo_executions_total[5m])) < 1000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: 'Low throughput in Ordo'
          description: 'Throughput is {{ $value }} ops/sec (expected: > 1000)'

  - name: ordo-capacity
    rules:
      - alert: OrdoHighMemoryUsage
        expr: |
          process_resident_memory_bytes{job="ordo"} 
          / node_memory_MemTotal_bytes * 100 > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'High memory usage in Ordo'
          description: 'Memory usage is {{ $value }}%'

      - alert: OrdoHighCPUUsage
        expr: |
          rate(process_cpu_seconds_total{job="ordo"}[5m]) * 100 > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'High CPU usage in Ordo'
          description: 'CPU usage is {{ $value }}%'

      - alert: OrdoTooManyOpenFiles
        expr: process_open_fds{job="ordo"} > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'Too many open file descriptors'
          description: 'Ordo has {{ $value }} open file descriptors'
```

## Alertmanager Configuration

```yaml
# alertmanager.yml
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'severity']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'critical'
    - match:
        severity: warning
      receiver: 'warning'

receivers:
  - name: 'default'
    webhook_configs:
      - url: 'http://your-webhook-endpoint'

  - name: 'critical'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/xxx'
        channel: '#alerts-critical'
        send_resolved: true
        title: '{{ .GroupLabels.alertname }}'
        text: "{{ range .Alerts }}{{ .Annotations.description }}\n{{ end }}"

  - name: 'warning'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/xxx'
        channel: '#alerts-warning'
        send_resolved: true

inhibit_rules:
  - source_match:
      severity: 'critical'
    target_match:
      severity: 'warning'
    equal: ['alertname']
```

## Recording Rules (Performance Optimization)

Pre-compute expensive queries:

```yaml
# recording-rules.yml
groups:
  - name: ordo-recording
    interval: 30s
    rules:
      - record: ordo:execution_rate:5m
        expr: sum(rate(ordo_executions_total[5m]))

      - record: ordo:error_rate:5m
        expr: |
          sum(rate(ordo_executions_total{result="error"}[5m])) 
          / sum(rate(ordo_executions_total[5m]))

      - record: ordo:latency_p99:5m
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          )

      - record: ordo:latency_p95:5m
        expr: |
          histogram_quantile(0.95, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          )

      - record: ordo:latency_p50:5m
        expr: |
          histogram_quantile(0.50, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          )

      - record: ordo:success_rate:5m
        expr: |
          sum(rate(ordo_executions_total{result="success"}[5m])) 
          / sum(rate(ordo_executions_total[5m]))
```

## Best Practices

1. **Set appropriate scrape interval**: 10-15 seconds for production
2. **Use recording rules**: Pre-compute common queries for faster dashboards
3. **Set up alerts**: Monitor error rate, latency, and availability
4. **Retain metrics**: Keep at least 15-30 days for trend analysis
5. **Label cardinality**: Avoid high-cardinality labels (e.g., request IDs)
6. **Resource limits**: Set memory limits for Prometheus in production
7. **Federation**: Use federation for multi-cluster monitoring
8. **Backup**: Regularly backup Prometheus data and Grafana dashboards

## Metric Labels

| Metric                              | Labels              | Description                   |
| ----------------------------------- | ------------------- | ----------------------------- |
| `ordo_info`                         | `version`           | Server version info           |
| `ordo_uptime_seconds`               | (none)              | Server uptime                 |
| `ordo_rules_total`                  | (none)              | Total loaded rules            |
| `ordo_executions_total`             | `ruleset`, `result` | Execution counter             |
| `ordo_execution_duration_seconds`   | `ruleset`           | Execution latency histogram   |
| `ordo_terminal_results_total`       | `ruleset`, `code`   | Terminal result distribution  |
| `ordo_eval_total`                   | `result`            | Expression evaluation counter |
| `ordo_eval_duration_seconds`        | (none)              | Expression evaluation latency |
| `ordo_active_executions`            | (none)              | Currently running executions  |
| `ordo_compilation_duration_seconds` | `ruleset`           | Rule compilation latency      |

## Troubleshooting

### Common Issues

**Metrics endpoint not responding:**

```bash
# Check if metrics are enabled
curl -v http://localhost:8080/metrics

# Check server logs
docker logs ordo 2>&1 | grep -i metric
```

**High cardinality warning:**

```promql
# Check label cardinality
count by (__name__) ({__name__=~"ordo.*"})
```

**Missing metrics:**

```bash
# Verify Prometheus target status
curl http://prometheus:9090/api/v1/targets | jq '.data.activeTargets[] | select(.labels.job=="ordo")'
```

## Related Documentation

- [Performance Benchmarks](/en/reference/benchmarks) - Detailed performance metrics
- [Configuration](/en/reference/configuration) - Server configuration options
- [Kubernetes Integration](/en/guide/integration/kubernetes) - K8s deployment guide
- [Nomad Integration](/en/guide/integration/nomad) - HashiCorp Nomad deployment

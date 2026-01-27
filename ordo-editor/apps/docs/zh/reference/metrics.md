# 指标与监控

Ordo 暴露了兼容 Prometheus 的指标，用于全面的监控、告警和性能分析。

## 快速开始

```bash
# 启动 Ordo 并启用指标 (默认启用)
ordo-server --metrics-enabled

# 访问指标端点
curl http://localhost:8080/metrics
```

## 端点

```
GET /metrics
```

返回 Prometheus 文本格式的指标。

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

## 监控栈部署

### Docker Compose (快速设置)

创建包含 Prometheus、Grafana 和 Alertmanager 的完整监控栈：

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

### Prometheus 配置文件

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

### Kubernetes 部署

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

#### PodMonitor (替代方案)

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

## 性能监控

### 关键性能指标

基于[基准测试结果](/zh/reference/benchmarks)，监控以下关键阈值：

| 指标         | 预期值        | 告警阈值     |
| ------------ | ------------- | ------------ |
| 简单规则执行 | ~350 ns       | > 1 µs       |
| 复杂规则执行 | ~2 µs         | > 10 µs      |
| 吞吐量       | ~2.8M ops/sec | < 1M ops/sec |
| 初始化时间   | ~50 ns        | > 1 µs       |
| 错误率       | < 0.1%        | > 1%         |
| P99 延迟     | < 5 ms        | > 50 ms      |

### 性能仪表板查询

**执行吞吐量 (ops/sec):**

```promql
sum(rate(ordo_executions_total[1m]))
```

**平均执行时间:**

```promql
rate(ordo_execution_duration_seconds_sum[5m])
/ rate(ordo_execution_duration_seconds_count[5m])
```

**P50/P95/P99 延迟:**

```promql
# P50
histogram_quantile(0.50, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))

# P95
histogram_quantile(0.95, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))

# P99
histogram_quantile(0.99, sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le))
```

**执行成功率:**

```promql
sum(rate(ordo_executions_total{result="success"}[5m]))
/ sum(rate(ordo_executions_total[5m])) * 100
```

## 完整 Grafana 仪表板

```json
{
  "title": "Ordo 规则引擎仪表板",
  "uid": "ordo-main",
  "tags": ["ordo", "rule-engine"],
  "timezone": "browser",
  "panels": [
    {
      "title": "吞吐量 (ops/sec)",
      "type": "stat",
      "gridPos": { "x": 0, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total[1m]))",
          "legendFormat": "总计"
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
      "title": "错误率",
      "type": "stat",
      "gridPos": { "x": 6, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "sum(rate(ordo_executions_total{result=\"error\"}[5m])) / sum(rate(ordo_executions_total[5m])) * 100",
          "legendFormat": "错误 %"
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
      "title": "P99 延迟",
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
      "title": "活跃规则数",
      "type": "stat",
      "gridPos": { "x": 18, "y": 0, "w": 6, "h": 4 },
      "targets": [
        {
          "expr": "ordo_rules_total",
          "legendFormat": "规则"
        }
      ]
    },
    {
      "title": "按规则集的请求速率",
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
      "title": "延迟分布",
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
      "title": "终端结果分布",
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
      "title": "按规则集的错误",
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

## 完整告警规则

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
          summary: 'Ordo 服务器宕机'
          description: 'Ordo 实例 {{ $labels.instance }} 已宕机超过 1 分钟。'

      - alert: OrdoHighRestartRate
        expr: changes(process_start_time_seconds{job="ordo"}[1h]) > 3
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'Ordo 频繁重启'
          description: 'Ordo 在过去一小时内重启了 {{ $value }} 次。'

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
          summary: 'Ordo 错误率过高'
          description: '错误率为 {{ $value | humanizePercentage }} (阈值: 1%)'

      - alert: OrdoCriticalErrorRate
        expr: |
          sum(rate(ordo_executions_total{result="error"}[5m])) 
          / sum(rate(ordo_executions_total[5m])) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: 'Ordo 错误率严重过高'
          description: '错误率为 {{ $value | humanizePercentage }} (阈值: 5%)'

      - alert: OrdoHighLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          ) > 0.01
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'Ordo 延迟过高'
          description: 'P99 延迟为 {{ $value | humanizeDuration }} (阈值: 10ms)'

      - alert: OrdoCriticalLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(ordo_execution_duration_seconds_bucket[5m])) by (le)
          ) > 0.05
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: 'Ordo 延迟严重过高'
          description: 'P99 延迟为 {{ $value | humanizeDuration }} (阈值: 50ms)'

      - alert: OrdoLowThroughput
        expr: sum(rate(ordo_executions_total[5m])) < 1000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: 'Ordo 吞吐量过低'
          description: '吞吐量为 {{ $value }} ops/sec (预期: > 1000)'

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
          summary: 'Ordo 内存使用过高'
          description: '内存使用率为 {{ $value }}%'

      - alert: OrdoHighCPUUsage
        expr: |
          rate(process_cpu_seconds_total{job="ordo"}[5m]) * 100 > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: 'Ordo CPU 使用过高'
          description: 'CPU 使用率为 {{ $value }}%'

      - alert: OrdoTooManyOpenFiles
        expr: process_open_fds{job="ordo"} > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: '打开的文件描述符过多'
          description: 'Ordo 有 {{ $value }} 个打开的文件描述符'
```

## Alertmanager 配置

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

## 记录规则 (性能优化)

预计算耗费资源的查询：

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

## 最佳实践

1. **设置合适的抓取间隔**：生产环境 10-15 秒
2. **使用记录规则**：预计算常见查询以加快仪表板速度
3. **设置告警**：监控错误率、延迟和可用性
4. **保留指标**：至少保留 15-30 天以进行趋势分析
5. **标签基数**：避免高基数标签 (如请求 ID)
6. **资源限制**：生产环境中为 Prometheus 设置内存限制
7. **联邦**：使用联邦进行多集群监控
8. **备份**：定期备份 Prometheus 数据和 Grafana 仪表板

## 指标标签

| 指标                                | 标签                | 描述                 |
| ----------------------------------- | ------------------- | -------------------- |
| `ordo_info`                         | `version`           | 服务器版本信息       |
| `ordo_uptime_seconds`               | (无)                | 服务器运行时间       |
| `ordo_rules_total`                  | (无)                | 已加载的规则总数     |
| `ordo_executions_total`             | `ruleset`, `result` | 执行计数器           |
| `ordo_execution_duration_seconds`   | `ruleset`           | 执行延迟直方图       |
| `ordo_terminal_results_total`       | `ruleset`, `code`   | 终端结果分布         |
| `ordo_eval_total`                   | `result`            | 表达式求值计数器     |
| `ordo_eval_duration_seconds`        | (无)                | 表达式求值延迟       |
| `ordo_active_executions`            | (无)                | 当前正在运行的执行数 |
| `ordo_compilation_duration_seconds` | `ruleset`           | 规则编译延迟         |

## 故障排除

### 常见问题

**指标端点无响应：**

```bash
# 检查是否启用了指标
curl -v http://localhost:8080/metrics

# 检查服务器日志
docker logs ordo 2>&1 | grep -i metric
```

**高基数警告：**

```promql
# 检查标签基数
count by (__name__) ({__name__=~"ordo.*"})
```

**缺少指标：**

```bash
# 验证 Prometheus 目标状态
curl http://prometheus:9090/api/v1/targets | jq '.data.activeTargets[] | select(.labels.job=="ordo")'
```

## 相关文档

- [性能基准](/zh/reference/benchmarks) - 详细性能指标
- [配置](/zh/reference/configuration) - 服务器配置选项
- [Kubernetes 集成](/zh/guide/integration/kubernetes) - K8s 部署指南
- [Nomad 集成](/zh/guide/integration/nomad) - HashiCorp Nomad 部署

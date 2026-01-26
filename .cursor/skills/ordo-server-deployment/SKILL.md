---
name: ordo-server-deployment
description: Ordo server deployment and configuration guide. Includes HTTP/gRPC configuration, environment variables, Nomad deployment, Docker, monitoring metrics, audit logging, multi-tenancy configuration. Use for production deployment, operations management, monitoring and alerting.
---

# Ordo Server Deployment and Configuration

## Quick Start

```bash
# Build
cargo build --release

# Basic startup
./target/release/ordo-server

# Startup with persistence
./target/release/ordo-server --rules-dir ./rules

# Full configuration
./target/release/ordo-server \
    --http-addr 0.0.0.0:8080 \
    --grpc-addr 0.0.0.0:50051 \
    --rules-dir ./rules \
    --audit-dir ./audit \
    --log-level info
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ORDO_HTTP_ADDR` | HTTP address | `0.0.0.0:8080` |
| `ORDO_GRPC_ADDR` | gRPC address | `0.0.0.0:50051` |
| `ORDO_UDS_PATH` | Unix Socket path | - |
| `ORDO_DISABLE_HTTP` | Disable HTTP | `false` |
| `ORDO_DISABLE_GRPC` | Disable gRPC | `false` |
| `ORDO_LOG_LEVEL` | Log level | `info` |
| `ORDO_RULES_DIR` | Rules persistence directory | - |
| `ORDO_MAX_VERSIONS` | Max version count | `10` |
| `ORDO_AUDIT_DIR` | Audit log directory | - |
| `ORDO_AUDIT_SAMPLE_RATE` | Audit sample rate (0-100) | `10` |
| `ORDO_DEBUG_MODE` | Debug mode | `false` |

### Multi-tenancy Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `ORDO_MULTI_TENANCY_ENABLED` | Enable multi-tenancy | `false` |
| `ORDO_DEFAULT_TENANT` | Default tenant ID | `default` |
| `ORDO_DEFAULT_TENANT_QPS` | Default QPS limit | - |
| `ORDO_DEFAULT_TENANT_BURST` | Default burst limit | - |
| `ORDO_DEFAULT_TENANT_TIMEOUT_MS` | Execution timeout (ms) | `100` |
| `ORDO_TENANTS_DIR` | Tenant config directory | - |

### Signature Verification Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `ORDO_SIGNATURE_ENABLED` | Enable signature verification | `false` |
| `ORDO_SIGNATURE_REQUIRE` | Reject unsigned rules | `false` |
| `ORDO_SIGNATURE_TRUSTED_KEYS` | Trusted public keys (comma-separated) | - |
| `ORDO_SIGNATURE_TRUSTED_KEYS_FILE` | Public key file path | - |
| `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | Allow unsigned local rules | `true` |

## Docker Deployment

### Dockerfile

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ordo-server /usr/local/bin/
EXPOSE 8080 50051
CMD ["ordo-server"]
```

### Run Container

```bash
docker run -d \
    -p 8080:8080 \
    -p 50051:50051 \
    -v $(pwd)/rules:/app/rules \
    -e ORDO_RULES_DIR=/app/rules \
    -e RUST_LOG=info \
    ghcr.io/pama-lee/ordo-server:latest
```

## Nomad Deployment

### Production Configuration

```hcl
job "ordo-server" {
  datacenters = ["dc1"]
  type = "service"
  
  update {
    max_parallel     = 1
    min_healthy_time = "10s"
    healthy_deadline = "3m"
    auto_revert      = true
  }
  
  group "ordo" {
    count = 1
    
    network {
      port "http" { static = 8080 }
      port "grpc" { static = 50051 }
    }
    
    service {
      name     = "ordo-http"
      port     = "http"
      provider = "nomad"
      
      check {
        name     = "http-health"
        type     = "http"
        path     = "/health"
        interval = "10s"
        timeout  = "3s"
      }
    }
    
    task "ordo-server" {
      driver = "docker"
      
      config {
        image = "ghcr.io/pama-lee/ordo-server:latest"
        ports = ["http", "grpc"]
      }
      
      env {
        RUST_LOG = "info"
      }
      
      resources {
        cpu    = 500
        memory = 256
      }
    }
  }
}
```

### Deployment Commands

```bash
cd deploy/nomad
nomad job run ordo-server.nomad
nomad job status ordo-server
```

## API Endpoints

### HTTP API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/metrics` | GET | Prometheus metrics |
| `/api/v1/rulesets` | GET | List all rules |
| `/api/v1/rulesets/:name` | GET/PUT/DELETE | Manage rules |
| `/api/v1/execute/:name` | POST | Execute rule |
| `/api/v1/rulesets/:name/versions` | GET | Version history |
| `/api/v1/rulesets/:name/rollback` | POST | Rollback version |

### Execute Rule

```bash
curl -X POST http://localhost:8080/api/v1/execute/discount-check \
    -H "Content-Type: application/json" \
    -d '{"input": {"user": {"vip": true}, "order": {"amount": 500}}}'
```

## Monitoring Metrics

### Prometheus Metrics

```
ordo_info{version="0.2.0"} 1
ordo_uptime_seconds 3600
ordo_rules_total 12
ordo_executions_total{ruleset="payment-check",result="success"} 1000
ordo_execution_duration_seconds_bucket{ruleset="payment-check",le="0.001"} 950
ordo_execution_duration_seconds_sum{ruleset="payment-check"} 1.5
```

### Grafana Dashboard Queries

```promql
# QPS
rate(ordo_executions_total[5m])

# Average latency
rate(ordo_execution_duration_seconds_sum[5m]) / rate(ordo_executions_total[5m])

# P99 latency
histogram_quantile(0.99, rate(ordo_execution_duration_seconds_bucket[5m]))

# Error rate
rate(ordo_executions_total{result="error"}[5m]) / rate(ordo_executions_total[5m])
```

## Audit Logging

### Enable Audit

```bash
./ordo-server --audit-dir ./audit --audit-sample-rate 10
```

### Log Format (JSON Lines)

```json
{"timestamp":"2024-01-08T10:00:00.123Z","level":"INFO","event":"server_started","version":"0.2.0"}
{"timestamp":"2024-01-08T10:00:01.456Z","level":"INFO","event":"rule_created","rule_name":"payment"}
{"timestamp":"2024-01-08T10:00:02.789Z","level":"INFO","event":"rule_executed","rule_name":"payment","duration_us":1500}
```

### Dynamic Sample Rate Adjustment

```bash
# Get current sample rate
curl http://localhost:8080/api/v1/config/audit-sample-rate

# Update sample rate
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
    -H "Content-Type: application/json" \
    -d '{"sample_rate": 50}'
```

## CLI Tools

### Key Generation

```bash
./ordo-keygen --output keys/
# Generates: keys/private.key, keys/public.key
```

### Rule Signing

```bash
./ordo-sign --key keys/private.key --input rule.json --output rule.signed.json
```

### Signature Verification

```bash
./ordo-verify --key keys/public.key --input rule.signed.json
```

## Key Files

- `crates/ordo-server/src/config.rs` - Server configuration
- `crates/ordo-server/src/api.rs` - HTTP API
- `crates/ordo-server/src/grpc.rs` - gRPC service
- `crates/ordo-server/src/metrics.rs` - Prometheus metrics
- `crates/ordo-server/src/audit.rs` - Audit logging
- `deploy/nomad/` - Nomad deployment configuration

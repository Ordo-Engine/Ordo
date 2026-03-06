# Configuration

This page documents all configuration options for Ordo server.

## Server Configuration

| Option           | CLI Flag                  | Default         | Description                                                     |
| ---------------- | ------------------------- | --------------- | --------------------------------------------------------------- |
| HTTP Address     | `--http-addr`             | `0.0.0.0:8080`  | HTTP server bind address                                        |
| gRPC Address     | `--grpc-addr`             | `0.0.0.0:50051` | gRPC server bind address                                        |
| UDS Path         | `--uds-path`              | None            | Unix Domain Socket path                                         |
| Disable HTTP     | `--disable-http`          | `false`         | Disable HTTP server                                             |
| Disable gRPC     | `--disable-grpc`          | `false`         | Disable gRPC server                                             |
| Log Level        | `--log-level`             | `info`          | Logging verbosity                                               |
| Shutdown Timeout | `--shutdown-timeout-secs` | `30`            | Seconds to wait for in-flight requests during graceful shutdown |
| Debug Mode       | `--debug-mode`            | `false`         | Enable debug API endpoints                                      |

## Storage Configuration

| Option          | CLI Flag         | Default | Description                    |
| --------------- | ---------------- | ------- | ------------------------------ |
| Rules Directory | `--rules-dir`    | None    | Directory for rule persistence |
| Max Versions    | `--max-versions` | `10`    | Historical versions per rule   |

## Audit Configuration

| Option          | CLI Flag              | Default | Description                 |
| --------------- | --------------------- | ------- | --------------------------- |
| Audit Directory | `--audit-dir`         | None    | Directory for audit logs    |
| Sample Rate     | `--audit-sample-rate` | `10`    | Execution sampling (0-100%) |

## Signature Configuration

| Option               | CLI Flag                           | Default | Description                                 |
| -------------------- | ---------------------------------- | ------- | ------------------------------------------- |
| Signature Enabled    | `--signature-enabled`              | `false` | Enable rule signature verification          |
| Require Signature    | `--signature-require`              | `false` | Reject unsigned rules on API updates        |
| Trusted Public Keys  | `--signature-trusted-keys`         | None    | Comma-separated base64 public keys          |
| Trusted Keys File    | `--signature-trusted-keys-file`    | None    | File with base64 public keys (one per line) |
| Allow Unsigned Local | `--signature-allow-unsigned-local` | `true`  | Allow unsigned local files on startup       |

## Deployment Configuration

| Option              | CLI Flag                     | Env Variable                 | Default      | Description                                                 |
| ------------------- | ---------------------------- | ---------------------------- | ------------ | ----------------------------------------------------------- |
| Instance Role       | `--role`                     | `ORDO_ROLE`                  | `standalone` | Instance role: `standalone`, `writer`, or `reader`          |
| Writer Address      | `--writer-addr`              | `ORDO_WRITER_ADDR`           | None         | Writer node address (used by reader to redirect writes)     |
| Watch Rules         | `--watch-rules`              | `ORDO_WATCH_RULES`           | `false`      | Enable file watcher for hot-reloading rules                 |
| Max Request Body    | `--max-request-body-bytes`   | `ORDO_MAX_REQUEST_BODY_BYTES`| `10485760`   | Maximum HTTP request body size in bytes (10 MB)             |
| Request Timeout     | `--request-timeout-secs`     | `ORDO_REQUEST_TIMEOUT_SECS`  | `30`         | HTTP request timeout in seconds                             |

### Writer/Reader Deployment

Ordo supports a distributed Writer/Reader deployment model for separating write and read traffic:

```bash
# Writer node — handles all rule CRUD operations
ordo-server --role writer --rules-dir /shared/rules --watch-rules

# Reader node — serves read and execution requests only
ordo-server --role reader \
  --writer-addr http://ordo-writer:8080 \
  --rules-dir /shared/rules \
  --watch-rules
```

When a reader receives a write request, it returns `409 Conflict` with the writer address:

```json
{
  "error": "This instance is read-only (role: reader)",
  "writer": "http://ordo-writer:8080",
  "hint": "Send write requests to the writer instance"
}
```

### File Watcher

When `--watch-rules` is enabled, Ordo monitors the rules directory for file changes:

- Uses native OS file system events (FSEvents on macOS, inotify on Linux)
- 200ms debounce to batch rapid file changes
- Falls back to 30-second polling if native events are unavailable
- In multi-tenancy mode, also watches `tenants.json` for tenant config changes

## Health Check Endpoints

Ordo provides Kubernetes-compatible health check endpoints:

| Endpoint         | Type      | Description                                            |
| ---------------- | --------- | ------------------------------------------------------ |
| `/healthz/live`  | Liveness  | Always returns `200` if the process is alive           |
| `/healthz/ready` | Readiness | Checks store lock availability and disk writability    |
| `/health`        | Readiness | Legacy endpoint, same behavior as `/healthz/ready`     |

### Readiness Checks

The readiness probe performs:
1. **Store lock** — Attempts to acquire a read lock with a 2-second timeout
2. **Disk writable** — Writes a `.health_probe` test file to `--rules-dir` (if configured)

```yaml
# Kubernetes probe configuration
livenessProbe:
  httpGet:
    path: /healthz/live
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
readinessProbe:
  httpGet:
    path: /healthz/ready
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 15
```

## Observability Configuration

| Option        | CLI Flag          | Env Variable         | Default       | Description                                                                                                    |
| ------------- | ----------------- | -------------------- | ------------- | -------------------------------------------------------------------------------------------------------------- |
| Service Name  | `--service-name`  | `ORDO_SERVICE_NAME`  | `ordo-server` | Service name reported in OpenTelemetry traces                                                                  |
| OTLP Endpoint | `--otlp-endpoint` | `ORDO_OTLP_ENDPOINT` | None          | OTLP HTTP endpoint for exporting traces (e.g. `http://localhost:4318`). If not set, OpenTelemetry is disabled. |

When `--otlp-endpoint` is set, traces are exported via OTLP HTTP (protobuf). This is compatible with OpenTelemetry Collector, Jaeger, Tempo, and other OTLP-compatible backends.

```bash
# Export traces to a local OpenTelemetry Collector
ordo-server \
  --service-name my-ordo \
  --otlp-endpoint http://otel-collector:4318
```

## Runtime Configuration

Some settings can be changed at runtime via API:

### Audit Sample Rate

```bash
# Get current rate
curl http://localhost:8080/api/v1/config/audit-sample-rate

# Update rate
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sample_rate": 50}'
```

## Docker Configuration

### Environment Variables

When running in Docker, you can use environment variables:

```dockerfile
ENV ORDO_HTTP_ADDR=0.0.0.0:8080
ENV ORDO_GRPC_ADDR=0.0.0.0:50051
ENV ORDO_RULES_DIR=/data/rules
ENV ORDO_AUDIT_DIR=/data/audit
ENV ORDO_LOG_LEVEL=info
ENV ORDO_SIGNATURE_ENABLED=true
ENV ORDO_SIGNATURE_TRUSTED_KEYS_FILE=/data/keys/trusted_keys.txt
ENV ORDO_SERVICE_NAME=ordo-server
ENV ORDO_OTLP_ENDPOINT=http://otel-collector:4318
ENV ORDO_SHUTDOWN_TIMEOUT_SECS=30
ENV ORDO_ROLE=standalone
ENV ORDO_WATCH_RULES=false
ENV ORDO_MAX_REQUEST_BODY_BYTES=10485760
ENV ORDO_REQUEST_TIMEOUT_SECS=30
```

### Docker Compose

```yaml
version: '3.8'

services:
  ordo:
    image: ordo-server:latest
    ports:
      - '8080:8080'
      - '50051:50051'
    volumes:
      - ./rules:/data/rules
      - ./audit:/data/audit
    command: >
      --http-addr 0.0.0.0:8080
      --grpc-addr 0.0.0.0:50051
      --rules-dir /data/rules
      --audit-dir /data/audit
      --audit-sample-rate 10
      --log-level info
```

## Kubernetes Configuration

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ordo-config
data:
  ORDO_HTTP_ADDR: '0.0.0.0:8080'
  ORDO_GRPC_ADDR: '0.0.0.0:50051'
  ORDO_LOG_LEVEL: 'info'
  ORDO_AUDIT_SAMPLE_RATE: '10'
  ORDO_SERVICE_NAME: 'ordo-server'
  ORDO_OTLP_ENDPOINT: 'http://otel-collector:4318'
  ORDO_SHUTDOWN_TIMEOUT_SECS: '30'
  ORDO_ROLE: 'standalone'
  ORDO_WATCH_RULES: 'true'
  ORDO_MAX_REQUEST_BODY_BYTES: '10485760'
  ORDO_REQUEST_TIMEOUT_SECS: '30'
```

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ordo-server
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: ordo
          image: ordo-server:latest
          args:
            - --http-addr=0.0.0.0:8080
            - --grpc-addr=0.0.0.0:50051
            - --rules-dir=/data/rules
            - --audit-dir=/data/audit
            - --watch-rules
          livenessProbe:
            httpGet:
              path: /healthz/live
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /healthz/ready
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 15
          volumeMounts:
            - name: rules
              mountPath: /data/rules
            - name: audit
              mountPath: /data/audit
      volumes:
        - name: rules
          persistentVolumeClaim:
            claimName: ordo-rules-pvc
        - name: audit
          emptyDir: {}
```

## Nomad Configuration

```hcl
job "ordo-server" {
  group "ordo" {
    task "server" {
      driver = "docker"

      config {
        image = "ordo-server:latest"
        args = [
          "--http-addr", "0.0.0.0:8080",
          "--grpc-addr", "0.0.0.0:50051",
          "--rules-dir", "/data/rules",
          "--audit-dir", "/data/audit",
          "--audit-sample-rate", "10",
        ]
      }

      resources {
        cpu    = 500
        memory = 256
      }
    }
  }
}
```

## Performance Tuning

### High Throughput

For maximum throughput:

```bash
ordo-server \
  --audit-sample-rate 1 \    # Minimal audit overhead
  --log-level warn           # Reduce logging
```

### Debugging

For troubleshooting:

```bash
ordo-server \
  --audit-sample-rate 100 \  # Log all executions
  --log-level debug          # Verbose logging
```

## Security Considerations

1. **Bind to localhost in development**: Use `127.0.0.1` instead of `0.0.0.0`
2. **Use TLS in production**: Configure reverse proxy with TLS
3. **Restrict audit log access**: Audit logs may contain sensitive data
4. **Set appropriate file permissions**: For rules and audit directories

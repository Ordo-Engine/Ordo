# Configuration

This page documents all configuration options for Ordo server.

## Server Configuration

| Option | CLI Flag | Default | Description |
|--------|----------|---------|-------------|
| HTTP Address | `--http-addr` | `0.0.0.0:8080` | HTTP server bind address |
| gRPC Address | `--grpc-addr` | `0.0.0.0:50051` | gRPC server bind address |
| UDS Path | `--uds-path` | None | Unix Domain Socket path |
| Disable HTTP | `--disable-http` | `false` | Disable HTTP server |
| Disable gRPC | `--disable-grpc` | `false` | Disable gRPC server |
| Log Level | `--log-level` | `info` | Logging verbosity |

## Storage Configuration

| Option | CLI Flag | Default | Description |
|--------|----------|---------|-------------|
| Rules Directory | `--rules-dir` | None | Directory for rule persistence |
| Max Versions | `--max-versions` | `10` | Historical versions per rule |

## Audit Configuration

| Option | CLI Flag | Default | Description |
|--------|----------|---------|-------------|
| Audit Directory | `--audit-dir` | None | Directory for audit logs |
| Sample Rate | `--audit-sample-rate` | `10` | Execution sampling (0-100%) |

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
```

### Docker Compose

```yaml
version: '3.8'

services:
  ordo:
    image: ordo-server:latest
    ports:
      - "8080:8080"
      - "50051:50051"
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
  HTTP_ADDR: "0.0.0.0:8080"
  GRPC_ADDR: "0.0.0.0:50051"
  LOG_LEVEL: "info"
  AUDIT_SAMPLE_RATE: "10"
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

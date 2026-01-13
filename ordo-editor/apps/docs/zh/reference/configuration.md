# 配置

本文档记录了 Ordo 服务器的所有配置选项。

## 服务器配置

| 选项      | CLI 标志         | 默认值          | 描述                |
| --------- | ---------------- | --------------- | ------------------- |
| HTTP 地址 | `--http-addr`    | `0.0.0.0:8080`  | HTTP 服务器绑定地址 |
| gRPC 地址 | `--grpc-addr`    | `0.0.0.0:50051` | gRPC 服务器绑定地址 |
| UDS 路径  | `--uds-path`     | 无              | Unix 域套接字路径   |
| 禁用 HTTP | `--disable-http` | `false`         | 禁用 HTTP 服务器    |
| 禁用 gRPC | `--disable-grpc` | `false`         | 禁用 gRPC 服务器    |
| 日志级别  | `--log-level`    | `info`          | 日志详细程度        |

## 存储配置

| 选项       | CLI 标志         | 默认值 | 描述                     |
| ---------- | ---------------- | ------ | ------------------------ |
| 规则目录   | `--rules-dir`    | 无     | 规则持久化目录           |
| 最大版本数 | `--max-versions` | `10`   | 每个规则保留的历史版本数 |

## 审计配置

| 选项     | CLI 标志              | 默认值 | 描述                |
| -------- | --------------------- | ------ | ------------------- |
| 审计目录 | `--audit-dir`         | 无     | 审计日志目录        |
| 采样率   | `--audit-sample-rate` | `10`   | 执行采样率 (0-100%) |

## 运行时配置

某些设置可以通过 API 在运行时更改：

### 审计采样率

```bash
# 获取当前采样率
curl http://localhost:8080/api/v1/config/audit-sample-rate

# 更新采样率
curl -X PUT http://localhost:8080/api/v1/config/audit-sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sample_rate": 50}'
```

## Docker 配置

### 环境变量

在 Docker 中运行时，可以使用环境变量：

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

## Kubernetes 配置

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ordo-config
data:
  HTTP_ADDR: '0.0.0.0:8080'
  GRPC_ADDR: '0.0.0.0:50051'
  LOG_LEVEL: 'info'
  AUDIT_SAMPLE_RATE: '10'
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

## Nomad 配置

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

## 性能调优

### 高吞吐量

为了获得最大吞吐量：

```bash
ordo-server \
  --audit-sample-rate 1 \    # 最小化审计开销
  --log-level warn           # 减少日志记录
```

### 调试

用于故障排除：

```bash
ordo-server \
  --audit-sample-rate 100 \  # 记录所有执行
  --log-level debug          # 详细日志记录
```

## 安全注意事项

1.  **开发环境中绑定到 localhost**：使用 `127.0.0.1` 而不是 `0.0.0.0`
2.  **生产环境使用 TLS**：配置带有 TLS 的反向代理
3.  **限制审计日志访问**：审计日志可能包含敏感数据
4.  **设置合适的文件权限**：针对规则和审计目录

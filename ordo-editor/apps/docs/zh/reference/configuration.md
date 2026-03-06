# 配置

本文档记录了 Ordo 服务器的所有配置选项。

## 服务器配置

| 选项         | CLI 标志                  | 默认值          | 描述                                 |
| ------------ | ------------------------- | --------------- | ------------------------------------ |
| HTTP 地址    | `--http-addr`             | `0.0.0.0:8080`  | HTTP 服务器绑定地址                  |
| gRPC 地址    | `--grpc-addr`             | `0.0.0.0:50051` | gRPC 服务器绑定地址                  |
| UDS 路径     | `--uds-path`              | 无              | Unix 域套接字路径                    |
| 禁用 HTTP    | `--disable-http`          | `false`         | 禁用 HTTP 服务器                     |
| 禁用 gRPC    | `--disable-grpc`          | `false`         | 禁用 gRPC 服务器                     |
| 日志级别     | `--log-level`             | `info`          | 日志详细程度                         |
| 优雅停机超时 | `--shutdown-timeout-secs` | `30`            | 优雅停机期间等待进行中请求完成的秒数 |
| 调试模式     | `--debug-mode`            | `false`         | 启用调试 API 端点                    |

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

## 签名配置

| 选项           | CLI 标志                           | 默认值  | 描述                        |
| -------------- | ---------------------------------- | ------- | --------------------------- |
| 启用签名验证   | `--signature-enabled`              | `false` | 启用规则签名验证            |
| 强制签名       | `--signature-require`              | `false` | API 更新时拒绝未签名规则    |
| 信任公钥列表   | `--signature-trusted-keys`         | 无      | 逗号分隔的 base64 公钥      |
| 公钥文件       | `--signature-trusted-keys-file`    | 无      | 公钥文件（每行一个 base64） |
| 允许本地无签名 | `--signature-allow-unsigned-local` | `true`  | 启动时允许本地规则无签名    |

## 部署配置

| 选项           | CLI 标志                   | 环境变量                      | 默认值       | 描述                                         |
| -------------- | -------------------------- | ----------------------------- | ------------ | -------------------------------------------- |
| 实例角色       | `--role`                   | `ORDO_ROLE`                   | `standalone` | 实例角色：`standalone`、`writer` 或 `reader` |
| Writer 地址    | `--writer-addr`            | `ORDO_WRITER_ADDR`            | 无           | Writer 节点地址（Reader 用于重定向写请求）   |
| 监控规则       | `--watch-rules`            | `ORDO_WATCH_RULES`            | `false`      | 启用文件监控实现规则热重载                   |
| 请求体大小限制 | `--max-request-body-bytes` | `ORDO_MAX_REQUEST_BODY_BYTES` | `10485760`   | HTTP 请求体最大字节数（10 MB）               |
| 请求超时       | `--request-timeout-secs`   | `ORDO_REQUEST_TIMEOUT_SECS`   | `30`         | HTTP 请求超时时间（秒）                      |

### Writer/Reader 部署

Ordo 支持 Writer/Reader 分布式部署模式，分离读写流量：

```bash
# Writer 节点 — 处理所有规则 CRUD 操作
ordo-server --role writer --rules-dir /shared/rules --watch-rules

# Reader 节点 — 仅提供读取和执行请求
ordo-server --role reader \
  --writer-addr http://ordo-writer:8080 \
  --rules-dir /shared/rules \
  --watch-rules
```

当 Reader 收到写请求时，返回 `409 Conflict` 并附带 Writer 地址：

```json
{
  "error": "This instance is read-only (role: reader)",
  "writer": "http://ordo-writer:8080",
  "hint": "Send write requests to the writer instance"
}
```

### 文件监控

启用 `--watch-rules` 后，Ordo 会监控规则目录的文件变更：

- 使用原生操作系统文件事件（macOS 上的 FSEvents，Linux 上的 inotify）
- 200ms 防抖处理快速连续变更
- 原生事件不可用时回退到 30 秒轮询
- 多租户模式下，同时监控 `tenants.json` 的租户配置变更

## 健康检查端点

Ordo 提供 Kubernetes 兼容的健康检查端点：

| 端点             | 类型     | 描述                                   |
| ---------------- | -------- | -------------------------------------- |
| `/healthz/live`  | 存活探针 | 进程存活则始终返回 `200`               |
| `/healthz/ready` | 就绪探针 | 检查 store 锁可用性和磁盘可写性        |
| `/health`        | 就绪探针 | 旧版端点，行为与 `/healthz/ready` 相同 |

### 就绪检查

就绪探针执行以下检查：

1. **Store 锁** — 尝试在 2 秒超时内获取读锁
2. **磁盘可写** — 向 `--rules-dir` 写入 `.health_probe` 测试文件（如已配置）

```yaml
# Kubernetes 探针配置
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

## 可观测性配置

| 选项      | CLI 标志          | 环境变量             | 默认值        | 描述                                                                                        |
| --------- | ----------------- | -------------------- | ------------- | ------------------------------------------------------------------------------------------- |
| 服务名称  | `--service-name`  | `ORDO_SERVICE_NAME`  | `ordo-server` | OpenTelemetry 链路追踪中上报的服务名称                                                      |
| OTLP 端点 | `--otlp-endpoint` | `ORDO_OTLP_ENDPOINT` | 无            | 导出链路追踪数据的 OTLP HTTP 端点（如 `http://localhost:4318`），未设置则禁用 OpenTelemetry |

设置 `--otlp-endpoint` 后，链路数据通过 OTLP HTTP（protobuf）导出，兼容 OpenTelemetry Collector、Jaeger、Tempo 等后端。

```bash
# 将链路数据导出到本地 OpenTelemetry Collector
ordo-server \
  --service-name my-ordo \
  --otlp-endpoint http://otel-collector:4318
```

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

## Kubernetes 配置

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

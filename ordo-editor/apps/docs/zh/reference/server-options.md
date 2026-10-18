# 服务参数

`ordo-server` 的参数。每个参数也可以用表中的环境变量设置。`ordo-server --help` 会打印同样的列表。开发用的 `ordo` 命令行见[命令行](/zh/platform/cli)。

## 监听

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--http-addr` | `ORDO_HTTP_ADDR` | `0.0.0.0:8080` | HTTP 地址 |
| `-p`、`--port` | `ORDO_PORT` | | 只设 HTTP 端口。两者都设时以 `--http-addr` 为准 |
| `--grpc-addr` | `ORDO_GRPC_ADDR` | `0.0.0.0:50051` | gRPC 地址 |
| `--grpc-port` | `ORDO_GRPC_PORT` | | 只设 gRPC 端口 |
| `--uds-path` | `ORDO_UDS_PATH` | 关闭 | 同时在这个 Unix Socket 上提供 gRPC |
| `--disable-http` | `ORDO_DISABLE_HTTP` | | 关闭 HTTP |
| `--disable-grpc` | `ORDO_DISABLE_GRPC` | | 关闭 gRPC |

## 规则文件

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--rules-dir` | `ORDO_RULES_DIR` | 只在内存 | 从这个目录加载规则，修改也保存到这里 |
| `--max-versions` | `ORDO_MAX_VERSIONS` | `10` | 每条规则保留的旧版本数 |
| `--watch-rules` | `ORDO_WATCH_RULES` | 关闭 | 磁盘上的文件变了就重新加载 |
| `--wal-dir` | `ORDO_WAL_DIR` | `<rules-dir>/wal` | 预写日志目录 |
| `--wal-disabled` | `ORDO_WAL_DISABLED` | | 关闭预写日志。写入在崩溃时不再安全 |
| `--wal-max-segment-bytes` | `ORDO_WAL_MAX_SEGMENT_BYTES` | `67108864` | 日志段达到这个大小就轮转 |
| `--wal-max-closed-segments` | `ORDO_WAL_MAX_CLOSED_SEGMENTS` | `3` | 保留的已关闭日志段数 |
| `--max-rules-per-tenant` | `ORDO_MAX_RULES_PER_TENANT` | 不限 | 超过后新建返回 `422` |
| `--max-total-rules` | `ORDO_MAX_TOTAL_RULES` | 不限 | 同上，所有租户合计 |

## 多实例

见[部署](/zh/guide/deployment#多实例)。

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--role` | `ORDO_ROLE` | `standalone` | `standalone`、`writer` 或 `reader`。读节点对写请求返回 `409` |
| `--writer-addr` | `ORDO_WRITER_ADDR` | | 写节点地址，读节点在 `409` 里返回它 |
| `--nats-url` | `ORDO_NATS_URL` | 关闭 | 通过 NATS JetStream 同步规则变更 |
| `--nats-subject-prefix` | `ORDO_NATS_SUBJECT_PREFIX` | `ordo.rules` | 同步事件的 subject 前缀 |
| `--instance-id` | `ORDO_INSTANCE_ID` | 随机 | 这个实例在 NATS 里的唯一名称 |

## 访问和限制

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--api-token` | `ORDO_API_TOKEN` | 关闭 | HTTP 数据接口需要 `Authorization: Bearer <token>` 或 `X-API-Token` |
| `--cors-allowed-origins` | `ORDO_CORS_ORIGINS` | 无 | 允许的浏览器来源，逗号分隔，或 `*` |
| `--max-concurrent-executions` | `ORDO_MAX_CONCURRENT_EXECUTIONS` | `0`（不限） | 同时处理的请求数，HTTP 和 gRPC 合计 |
| `--request-timeout-secs` | `ORDO_REQUEST_TIMEOUT_SECS` | `30` | HTTP 请求超时后返回 `408` |
| `--max-request-body-bytes` | `ORDO_MAX_REQUEST_BODY_BYTES` | `10485760` | 请求体上限，HTTP 和 gRPC |
| `--shutdown-timeout-secs` | `ORDO_SHUTDOWN_TIMEOUT_SECS` | `30` | 关闭时等待进行中请求的时间 |
| `--grpc-tls-enabled` | `ORDO_GRPC_TLS_ENABLED` | | gRPC 的 TLS，配合下面两个参数 |
| `--grpc-tls-cert` | `ORDO_GRPC_TLS_CERT` | | PEM 证书 |
| `--grpc-tls-key` | `ORDO_GRPC_TLS_KEY` | | PEM 私钥（PKCS8） |
| `--grpc-mtls-enabled` | `ORDO_GRPC_MTLS_ENABLED` | | 要求客户端证书，同时开启 TLS |
| `--grpc-tls-client-ca` | `ORDO_GRPC_TLS_CLIENT_CA` | | 签发客户端证书的 CA |

## 租户

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--multi-tenancy-enabled` | `ORDO_MULTI_TENANCY_ENABLED` | 关闭 | 按 `X-Tenant-ID` 分开规则和限制 |
| `--default-tenant` | `ORDO_DEFAULT_TENANT` | `default` | 没带请求头时用的租户 |
| `--default-tenant-qps` | `ORDO_DEFAULT_TENANT_QPS` | 不限 | 该租户每秒请求数 |
| `--default-tenant-burst` | `ORDO_DEFAULT_TENANT_BURST` | 不限 | 该租户的突发量 |
| `--default-tenant-timeout-ms` | `ORDO_DEFAULT_TENANT_TIMEOUT_MS` | `100` | 该租户的执行超时 |
| `--tenants-dir` | `ORDO_TENANTS_DIR` | `<rules-dir>/tenants/tenants.json` | 保存租户设置的文件 |

## 签名规则

见[签名规则](/zh/guide/deployment#签名规则)。

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--signature-enabled` | `ORDO_SIGNATURE_ENABLED` | 关闭 | 校验签名 |
| `--signature-require` | `ORDO_SIGNATURE_REQUIRE` | 关闭 | 拒绝未签名的规则，API 和文件都算 |
| `--signature-trusted-keys` | `ORDO_SIGNATURE_TRUSTED_KEYS` | | 受信任的公钥，base64，逗号分隔 |
| `--signature-trusted-keys-file` | `ORDO_SIGNATURE_TRUSTED_KEYS_FILE` | | 同上，每行一个 |
| `--signature-allow-unsigned-local` | `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | `true` | 加载未签名的本地文件。只能用环境变量关闭：`ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL=false` |

## 日志、审计和捕获

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--log-level` | `ORDO_LOG_LEVEL` | `info` | `trace`、`debug`、`info`、`warn` 或 `error` |
| `--log-format` | `ORDO_LOG_FORMAT` | `text` | `text` 或 `json` |
| `--audit-dir` | `ORDO_AUDIT_DIR` | 只输出到 stdout | 同时把审计日志按天写到这个目录 |
| `--audit-sample-rate` | `ORDO_AUDIT_SAMPLE_RATE` | `10` | 写入审计日志的执行百分比 |
| `--capture-io-path` | `ORDO_CAPTURE_IO_PATH` | 关闭 | 记录每次执行的输入和输出，供 `ordo replay` 和 `ordo impact` 使用 |
| `--capture-io-sample-rate` | `ORDO_CAPTURE_IO_SAMPLE_RATE` | `100` | 记录的执行百分比 |
| `--service-name` | `ORDO_SERVICE_NAME` | `ordo-server` | 追踪里的服务名 |
| `--otlp-endpoint` | `ORDO_OTLP_ENDPOINT` | 关闭 | 通过 OTLP HTTP 导出 OpenTelemetry 追踪，比如 `http://otel-collector:4318` |
| `--debug-mode` | `ORDO_DEBUG_MODE` | 关闭 | 加上调试 API。不要用于生产 |

## 决策模型网关

见[决策模型网关](/zh/platform/decision-gateway)。

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--decide-upstream` | `ORDO_DECIDE_UPSTREAM` | 关闭 | 要转发到的 System One 兼容 API。设置后网关开启 |
| `--decide-api-key` | `ORDO_DECIDE_API_KEY` | | 发给上游的密钥。不设时转发调用方的 `Authorization`。开启 `--api-token` 时必须设置 |
| `--decide-log-dir` | `ORDO_DECIDE_LOG_DIR` | 关闭 | 决策日志写到这里 |
| `--decide-log-requests` | `ORDO_DECIDE_LOG_REQUESTS` | 关闭 | 同时记录每个请求的 `state` 和 `questions` |

## 平台

见[引擎](/zh/platform/servers)。

| 参数 | 环境变量 | 默认值 | 作用 |
| --- | --- | --- | --- |
| `--platform-url` | `ORDO_PLATFORM_URL` | | 要注册到的平台 |
| `--server-token` | `ORDO_SERVER_TOKEN` | | 这个引擎唯一的令牌，注册时必需 |
| `--platform-connect-token` | `ORDO_CONNECT_TOKEN` | | 把引擎放进一个组织 |
| `--platform-registration-secret` | `ORDO_PLATFORM_REGISTRATION_SECRET` | | 不用连接令牌时的全平台密钥 |
| `--server-name` | `ORDO_SERVER_NAME` | `ordo-server` | 平台里显示的名称 |
| `--server-url` | `ORDO_SERVER_URL` | `http://<http-addr>` | 平台访问这个引擎的地址 |
| `--stats-interval-secs` | `ORDO_STATS_INTERVAL_SECS` | `15` | 向平台发送执行统计的间隔 |

## 签名工具

`cargo build --release` 还会构建 `ordo-keygen`、`ordo-sign` 和 `ordo-verify`：

```bash
ordo-keygen --output ./keys
ordo-sign --key ./keys/private.key --input rule.json [--output rule.signed.json]
ordo-verify --key ./keys/public.key --input rule.signed.json
```

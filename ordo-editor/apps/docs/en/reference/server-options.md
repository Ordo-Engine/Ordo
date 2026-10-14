# Server Options

Options of `ordo-server`. Every option can also be set with the environment variable shown. `ordo-server --help` prints the same list. For the `ordo` developer CLI, see [CLI](/en/platform/cli).

## Listening

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--http-addr` | `ORDO_HTTP_ADDR` | `0.0.0.0:8080` | HTTP address |
| `-p`, `--port` | `ORDO_PORT` | | HTTP port only. `--http-addr` wins if both are set |
| `--grpc-addr` | `ORDO_GRPC_ADDR` | `0.0.0.0:50051` | gRPC address |
| `--grpc-port` | `ORDO_GRPC_PORT` | | gRPC port only |
| `--uds-path` | `ORDO_UDS_PATH` | off | Also serve gRPC on this Unix socket |
| `--disable-http` | `ORDO_DISABLE_HTTP` | | Turn HTTP off |
| `--disable-grpc` | `ORDO_DISABLE_GRPC` | | Turn gRPC off |

## Rules on disk

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--rules-dir` | `ORDO_RULES_DIR` | memory only | Load rules from this folder and save changes to it |
| `--max-versions` | `ORDO_MAX_VERSIONS` | `10` | Old versions kept per rule |
| `--watch-rules` | `ORDO_WATCH_RULES` | off | Reload files that change on disk |
| `--wal-dir` | `ORDO_WAL_DIR` | `<rules-dir>/wal` | Write-ahead log folder |
| `--wal-disabled` | `ORDO_WAL_DISABLED` | | Turn the write-ahead log off. Writes are then not crash safe |
| `--wal-max-segment-bytes` | `ORDO_WAL_MAX_SEGMENT_BYTES` | `67108864` | Size at which a log segment is rotated |
| `--wal-max-closed-segments` | `ORDO_WAL_MAX_CLOSED_SEGMENTS` | `3` | Closed segments kept |
| `--max-rules-per-tenant` | `ORDO_MAX_RULES_PER_TENANT` | no limit | Further creates get `422` |
| `--max-total-rules` | `ORDO_MAX_TOTAL_RULES` | no limit | Same, across all tenants |

## Several instances

See [Deployment](/en/guide/deployment#several-instances).

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--role` | `ORDO_ROLE` | `standalone` | `standalone`, `writer` or `reader`. Readers reject writes with `409` |
| `--writer-addr` | `ORDO_WRITER_ADDR` | | Writer address, returned by readers in the `409` |
| `--nats-url` | `ORDO_NATS_URL` | off | Sync rule changes through NATS JetStream |
| `--nats-subject-prefix` | `ORDO_NATS_SUBJECT_PREFIX` | `ordo.rules` | Subject prefix for sync events |
| `--instance-id` | `ORDO_INSTANCE_ID` | random | Unique name of this instance for NATS |

## Access and limits

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--api-token` | `ORDO_API_TOKEN` | off | HTTP data routes need `Authorization: Bearer <token>` or `X-API-Token` |
| `--cors-allowed-origins` | `ORDO_CORS_ORIGINS` | none | Allowed browser origins, comma-separated, or `*` |
| `--max-concurrent-executions` | `ORDO_MAX_CONCURRENT_EXECUTIONS` | `0` (no limit) | Requests in flight at once, HTTP and gRPC together |
| `--request-timeout-secs` | `ORDO_REQUEST_TIMEOUT_SECS` | `30` | HTTP requests that take longer get `408` |
| `--max-request-body-bytes` | `ORDO_MAX_REQUEST_BODY_BYTES` | `10485760` | Largest request body, HTTP and gRPC |
| `--shutdown-timeout-secs` | `ORDO_SHUTDOWN_TIMEOUT_SECS` | `30` | How long shutdown waits for requests in flight |
| `--grpc-tls-enabled` | `ORDO_GRPC_TLS_ENABLED` | | TLS for gRPC, with the two options below |
| `--grpc-tls-cert` | `ORDO_GRPC_TLS_CERT` | | PEM certificate |
| `--grpc-tls-key` | `ORDO_GRPC_TLS_KEY` | | PEM private key (PKCS8) |
| `--grpc-mtls-enabled` | `ORDO_GRPC_MTLS_ENABLED` | | Require client certificates. Implies TLS |
| `--grpc-tls-client-ca` | `ORDO_GRPC_TLS_CLIENT_CA` | | CA that signs client certificates |

## Tenants

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--multi-tenancy-enabled` | `ORDO_MULTI_TENANCY_ENABLED` | off | Separate rules and limits per `X-Tenant-ID` |
| `--default-tenant` | `ORDO_DEFAULT_TENANT` | `default` | Tenant for requests without the header |
| `--default-tenant-qps` | `ORDO_DEFAULT_TENANT_QPS` | no limit | Requests per second for that tenant |
| `--default-tenant-burst` | `ORDO_DEFAULT_TENANT_BURST` | no limit | Burst size for that tenant |
| `--default-tenant-timeout-ms` | `ORDO_DEFAULT_TENANT_TIMEOUT_MS` | `100` | Execution timeout for that tenant |
| `--tenants-dir` | `ORDO_TENANTS_DIR` | `<rules-dir>/tenants/tenants.json` | File where tenant settings are stored |

## Signed rules

See [Signed rules](/en/guide/deployment#signed-rules).

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--signature-enabled` | `ORDO_SIGNATURE_ENABLED` | off | Check signatures |
| `--signature-require` | `ORDO_SIGNATURE_REQUIRE` | off | Reject unsigned rules, from the API and from files |
| `--signature-trusted-keys` | `ORDO_SIGNATURE_TRUSTED_KEYS` | | Trusted public keys, base64, comma-separated |
| `--signature-trusted-keys-file` | `ORDO_SIGNATURE_TRUSTED_KEYS_FILE` | | The same, one per line |
| `--signature-allow-unsigned-local` | `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | `true` | Load unsigned local files. Only the variable can turn it off: `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL=false` |

## Logs, audit and capture

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--log-level` | `ORDO_LOG_LEVEL` | `info` | `trace`, `debug`, `info`, `warn` or `error` |
| `--log-format` | `ORDO_LOG_FORMAT` | `text` | `text` or `json` |
| `--audit-dir` | `ORDO_AUDIT_DIR` | stdout only | Also write the audit log to daily files here |
| `--audit-sample-rate` | `ORDO_AUDIT_SAMPLE_RATE` | `10` | Percent of executions in the audit log |
| `--capture-io-path` | `ORDO_CAPTURE_IO_PATH` | off | Record each execution's input and output, for `ordo replay` and `ordo impact` |
| `--capture-io-sample-rate` | `ORDO_CAPTURE_IO_SAMPLE_RATE` | `100` | Percent of executions recorded |
| `--service-name` | `ORDO_SERVICE_NAME` | `ordo-server` | Service name in traces |
| `--otlp-endpoint` | `ORDO_OTLP_ENDPOINT` | off | Export OpenTelemetry traces over OTLP HTTP, such as `http://otel-collector:4318` |
| `--debug-mode` | `ORDO_DEBUG_MODE` | off | Add the debug API. Not for production |

## Decision model gateway

See [Decision Model Gateway](/en/platform/decision-gateway).

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--decide-upstream` | `ORDO_DECIDE_UPSTREAM` | off | System One compatible API to forward to. Turns the gateway on |
| `--decide-api-key` | `ORDO_DECIDE_API_KEY` | | Key sent upstream. Without it the caller's `Authorization` is forwarded. Needed with `--api-token` |
| `--decide-log-dir` | `ORDO_DECIDE_LOG_DIR` | off | Write the decision log here |
| `--decide-log-requests` | `ORDO_DECIDE_LOG_REQUESTS` | off | Also log each request's `state` and `questions` |

## Platform

See [Engines](/en/platform/servers).

| Option | Environment | Default | What it does |
| --- | --- | --- | --- |
| `--platform-url` | `ORDO_PLATFORM_URL` | | Platform to register with |
| `--server-token` | `ORDO_SERVER_TOKEN` | | Unique token of this engine. Needed to register |
| `--platform-connect-token` | `ORDO_CONNECT_TOKEN` | | Puts the engine in an organization |
| `--platform-registration-secret` | `ORDO_PLATFORM_REGISTRATION_SECRET` | | Platform-wide secret, when not using connect tokens |
| `--server-name` | `ORDO_SERVER_NAME` | `ordo-server` | Name shown in the platform |
| `--server-url` | `ORDO_SERVER_URL` | `http://<http-addr>` | Address the platform uses to reach this engine |
| `--stats-interval-secs` | `ORDO_STATS_INTERVAL_SECS` | `15` | How often execution stats are sent to the platform |

## Signing tools

`cargo build --release` also builds `ordo-keygen`, `ordo-sign` and `ordo-verify`:

```bash
ordo-keygen --output ./keys
ordo-sign --key ./keys/private.key --input rule.json [--output rule.signed.json]
ordo-verify --key ./keys/public.key --input rule.signed.json
```

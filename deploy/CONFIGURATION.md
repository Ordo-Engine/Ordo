# Ordo Configuration Reference

Every `ORDO_*` environment variable accepted by the two server binaries, with
its default and purpose. Each variable has an equivalent CLI flag (shown in the
binary's `--help`); environment variables are overridden by an explicit flag.

Generated from the `clap` definitions in:

- `crates/ordo-server/src/config.rs` (`ServerConfig`)
- `crates/ordo-platform/src/config.rs` (`PlatformConfig`)

Conventions in the tables below:

- **Default `—`** = unset / `None`. The "Purpose" column notes the effective
  behaviour when unset where it differs.
- Boolean flags accept `true` / `false`.
- List values (marked *(csv)*) are comma-separated.

---

## ordo-server (rule engine)

### Network & transports

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_HTTP_ADDR` | `0.0.0.0:8080` | HTTP REST listen address. |
| `ORDO_PORT` | — | Shorthand for HTTP port; ignored if `ORDO_HTTP_ADDR` is set. Effective HTTP default is `8080`. |
| `ORDO_GRPC_ADDR` | `0.0.0.0:50051` | gRPC listen address. |
| `ORDO_GRPC_PORT` | — | Shorthand for gRPC port; ignored if `ORDO_GRPC_ADDR` is set. Effective gRPC default is `50051`. |
| `ORDO_UDS_PATH` | — | Unix Domain Socket path for gRPC-over-UDS (Unix only). Disabled when unset. |
| `ORDO_DISABLE_HTTP` | `false` | Disable the HTTP server. |
| `ORDO_DISABLE_GRPC` | `false` | Disable the gRPC server. |

### Storage & persistence

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_RULES_DIR` | — | Directory for rule persistence. Unset = in-memory only (rules lost on restart). |
| `ORDO_MAX_VERSIONS` | `10` | Historical versions kept per rule (only with `ORDO_RULES_DIR`). |
| `ORDO_WAL_DIR` | `{rules-dir}/wal/` | Write-Ahead Log directory override (only with `ORDO_RULES_DIR`). |
| `ORDO_WAL_DISABLED` | `false` | Disable the WAL. Not recommended — crash-safe persistence is not guaranteed. |
| `ORDO_WAL_MAX_SEGMENT_BYTES` | `67108864` (64 MiB) | Max size of a single WAL segment before rotation. |
| `ORDO_WAL_MAX_CLOSED_SEGMENTS` | `3` | Closed WAL segments retained after rotation. |

### Distributed sync (NATS)

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_ROLE` | `standalone` | Instance role: `standalone`, `writer`, or `reader`. Readers reject writes. |
| `ORDO_WRITER_ADDR` | — | Writer address returned in 409s so readers can redirect writes. |
| `ORDO_WATCH_RULES` | `false` | Watch `ORDO_RULES_DIR` for live reload (requires `ORDO_RULES_DIR`). |
| `ORDO_NATS_URL` | — | NATS server URL (e.g. `nats://nats:4222`). Enables JetStream rule sync. Requires the `nats-sync` build feature. |
| `ORDO_NATS_SUBJECT_PREFIX` | `ordo.rules` | Subject prefix for sync events (stream `ordo-rules`, subjects `ordo.rules.>`). |
| `ORDO_INSTANCE_ID` | auto | Stable instance ID for NATS consumer naming. Defaults to `hostname:port`, else random hex. |

### Multi-tenancy

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_MULTI_TENANCY_ENABLED` | `false` | Enable per-tenant isolation (tenant via `X-Tenant-ID` / `x-tenant-id`). |
| `ORDO_DEFAULT_TENANT` | `default` | Default tenant ID. |
| `ORDO_DEFAULT_TENANT_QPS` | — | Default tenant QPS limit. Unset = unlimited. |
| `ORDO_DEFAULT_TENANT_BURST` | — | Default tenant burst limit. Unset = unlimited. |
| `ORDO_DEFAULT_TENANT_TIMEOUT_MS` | `100` | Default tenant execution timeout (ms). |
| `ORDO_TENANTS_DIR` | — | Tenant config directory. Defaults to `<rules-dir>/tenants.json` behaviour when unset. |
| `ORDO_MAX_RULES_PER_TENANT` | — | Max rulesets per tenant. Unset = unlimited. |
| `ORDO_MAX_TOTAL_RULES` | — | Max rulesets across all tenants. Unset = unlimited. |

### Platform registration

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_PLATFORM_URL` | — | ordo-platform URL to self-register + heartbeat against. Disabled when unset. |
| `ORDO_SERVER_NAME` | `ordo-server` | Human-readable name in the platform registry. |
| `ORDO_SERVER_TOKEN` | — | **Secret.** Shared token for platform registration (unique per server). |
| `ORDO_PLATFORM_REGISTRATION_SECRET` | — | **Secret.** Platform-wide registration secret; must match the platform side. |
| `ORDO_SERVER_URL` | — | Public URL of this server as seen by the platform. Defaults to `http://<http-addr>`. |

### Security: signatures & CORS

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_SIGNATURE_ENABLED` | `false` | Enable ED25519 signature verification on rule loads/updates. |
| `ORDO_SIGNATURE_REQUIRE` | `false` | Reject unsigned rules (only with signatures enabled). |
| `ORDO_SIGNATURE_TRUSTED_KEYS` | — | *(csv)* Trusted public keys (base64). |
| `ORDO_SIGNATURE_TRUSTED_KEYS_FILE` | — | File with one base64 public key per line. |
| `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL` | `true` | Allow unsigned local rule files when verification is enabled. |
| `ORDO_CORS_ORIGINS` | — | *(csv)* Allowed CORS origins (non-debug mode). Unset = cross-origin rejected. `*` allows all (avoid in prod). |

### Security: gRPC TLS / mTLS

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_GRPC_TLS_ENABLED` | `false` | Enable TLS for gRPC (requires cert + key). |
| `ORDO_GRPC_TLS_CERT` | — | PEM server certificate path. |
| `ORDO_GRPC_TLS_KEY` | — | PEM private key (PKCS8) path. |
| `ORDO_GRPC_MTLS_ENABLED` | `false` | Require client certs (mTLS); implies TLS enabled. |
| `ORDO_GRPC_TLS_CLIENT_CA` | — | PEM CA cert to verify client certificates (mTLS). |

### Observability & limits

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_LOG_LEVEL` | `info` | Log level: `trace`, `debug`, `info`, `warn`, `error`. |
| `ORDO_AUDIT_DIR` | — | Audit log directory (JSON Lines, daily rotation). Stdout logging always on. |
| `ORDO_AUDIT_SAMPLE_RATE` | `10` | Execution log sampling percent (0–100). Runtime-adjustable via API. |
| `ORDO_DEBUG_MODE` | `false` | Enable debug API endpoints + VM traces. **Never enable in production.** |
| `ORDO_SERVICE_NAME` | `ordo-server` | Service name in OpenTelemetry traces/logs. |
| `ORDO_OTLP_ENDPOINT` | — | OTLP HTTP endpoint (e.g. `http://otel:4318`). Unset = OpenTelemetry disabled. |
| `ORDO_SHUTDOWN_TIMEOUT_SECS` | `30` | Graceful shutdown timeout for in-flight requests. |
| `ORDO_MAX_REQUEST_BODY_BYTES` | `10485760` (10 MB) | Max HTTP request body (gRPC message limit set to same). |
| `ORDO_REQUEST_TIMEOUT_SECS` | `30` | HTTP request timeout (408 on exceed). |

> Health endpoints (not configurable): `GET /healthz/live`, `GET /healthz/ready`
> (`/health` is an alias of readiness), Prometheus metrics at `GET /metrics`.

---

## ordo-platform (control plane / gateway)

Also consumed by `ordo-platform-worker` (the background release-event processor),
except the HTTP listen address.

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORDO_PLATFORM_ADDR` | `0.0.0.0:3000` | HTTP listen address. **Note:** `compose.yml` and the K8s manifests run it on `:3001`. |
| `ORDO_DATABASE_URL` | **required** | PostgreSQL connection URL (e.g. `postgresql://user:pass@host:5432/ordo_platform`). Embedded sqlx migrations run on startup. |
| `ORDO_ENGINE_URL` | `http://localhost:8080` | ordo-server base URL for the engine proxy. |
| `ORDO_NATS_URL` | — | NATS URL for publishing tenant/ruleset/release sync events. Disabled when unset. |
| `ORDO_NATS_SUBJECT_PREFIX` | `ordo.rules` | NATS subject prefix (must match the server side). |
| `ORDO_INSTANCE_ID` | auto | Instance ID in sync envelopes. Defaults to `hostname:port`, else `platform-<hex>`. |
| `ORDO_JWT_SECRET` | **required** | **Secret.** JWT signing secret; must be ≥ 32 characters (startup fails otherwise). |
| `ORDO_JWT_EXPIRY_HOURS` | `24` | JWT token expiry in hours. |
| `ORDO_PLATFORM_CORS_ORIGINS` | `*` | *(csv)* Allowed CORS origins. **Set to real origins in production**, not `*`. |
| `ORDO_LOG_LEVEL` | `info` | Log level. |
| `ORDO_PLATFORM_TEMPLATES_DIR` | `./templates` | Rule template directory. Template system disabled if the path is absent. (Image bundles `/app/templates`.) |
| `ORDO_ALLOW_REGISTRATION` | `false` | Allow self-service account registration via `/register`. When false, users join only via org invite. |
| `ORDO_PLATFORM_REGISTRATION_SECRET` | — | **Secret.** Shared secret required (`X-Registration-Secret`) for ordo-server registration/heartbeats. Unset = unauthenticated registration (not recommended). |
| `ORDO_ALLOW_ORG_CREATION` | `false` | Allow authenticated users to create root-level orgs. When false, only platform admins can. |
| `ORDO_GITHUB_CLIENT_ID` | — | GitHub OAuth App client ID. Unset = GitHub OAuth disabled. |
| `ORDO_GITHUB_CLIENT_SECRET` | — | **Secret.** GitHub OAuth App client secret. |
| `ORDO_GITHUB_CALLBACK_URL` | `http://localhost:3000/api/v1/github/callback` | OAuth callback URL; must exactly match the GitHub OAuth App config. |

> Health endpoint: `GET /health` currently returns a `200 "ok"` stub as soon as
> the HTTP listener is up — it does **not** yet verify Postgres/NATS. A deeper
> readiness endpoint is being hardened separately.

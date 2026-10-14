# Running the Server

`ordo-server` runs rules for your applications. It serves HTTP on port 8080 and gRPC on port 50051, and can also listen on a Unix socket.

## Start it

With Docker:

```bash
docker run -p 8080:8080 -p 50051:50051 ghcr.io/ordo-engine/ordo:latest
curl http://localhost:8080/health
```

From source (Rust 1.83 or later):

```bash
git clone https://github.com/Ordo-Engine/Ordo.git && cd Ordo
cargo build --release
./target/release/ordo-server --rules-dir ./rules
```

Every flag also has an `ORDO_*` environment variable. See [Server Options](/en/reference/server-options).

## Keep rules on disk

Without `--rules-dir`, rules live in memory and are gone after a restart. With it:

- `.json`, `.yaml` and `.yml` files in the folder are loaded at startup. The file name is the rule name.
- Rules created, updated or deleted through the API are written to the folder.
- A write-ahead log in `<rules-dir>/wal/` makes writes crash safe.

```bash
docker run -p 8080:8080 -v "$PWD/rules:/data/rules" --user "$(id -u):$(id -g)" \
  ghcr.io/ordo-engine/ordo:latest --rules-dir /data/rules
```

The folder must be writable from inside the container. On Linux, `--user` runs the container as you so it can write to your folder.

To pick up files that change on disk (from git or a deploy script), add `--watch-rules`. To reload by hand, call `POST /api/v1/admin/reload`.

## Call a rule

```bash
curl -X POST http://localhost:8080/api/v1/rulesets \
  -H 'Content-Type: application/json' -d @rulesets/discount.json

curl -X POST http://localhost:8080/api/v1/execute/discount \
  -H 'Content-Type: application/json' \
  -d '{"input":{"user":{"tier":"gold"},"order":{"amount":1200}}}'
```

```json
{ "code": "OK", "message": "", "output": { "rate": 0.15, "pay": 1020.0 }, "duration_us": 12 }
```

Add `"trace": true` to see the path. For many inputs at once, use `POST /api/v1/execute/discount/batch` with `{"inputs": [...]}`. The [HTTP API](/en/api/http-api) and [gRPC API](/en/api/grpc-api) list everything.

Client libraries for Go, Python and Java are in [`sdk/`](https://github.com/Ordo-Engine/Ordo/tree/main/sdk) in the repository. They are not on the package registries yet.

## Versions and rollback

With `--rules-dir`, each update keeps the previous version next to the rule file (`discount.v1.json`, `discount.v2.json`, …). `--max-versions` (default 10) sets how many are kept.

```bash
curl http://localhost:8080/api/v1/rulesets/discount/versions
curl -X POST http://localhost:8080/api/v1/rulesets/discount/rollback \
  -H 'Content-Type: application/json' -d '{"seq": 1}'
```

```json
{ "status": "rolled_back", "name": "discount", "from_version": "1.1.0", "to_version": "1.0.0" }
```

Without `--rules-dir` there is no history and rollback returns an error.

## Health checks

| Endpoint         | Use                                                          |
| ---------------- | ------------------------------------------------------------ |
| `/healthz/live`  | Liveness: `200` while the process runs                        |
| `/healthz/ready` | Readiness: checks the rule store and that `--rules-dir` is writable |
| `/health`        | Same as `/healthz/ready`                                      |
| `/metrics`       | Prometheus metrics. See [Metrics](/en/reference/metrics)      |

These are never behind the API token or the concurrency limit.

## Audit log

Rule changes and a sample of executions are written as JSON lines to stdout, and to daily files when `--audit-dir` is set:

```bash
ordo-server --rules-dir ./rules --audit-dir ./audit --audit-sample-rate 10
```

```json
{"timestamp":"…","level":"INFO","event":"rule_updated","rule_name":"discount","from_version":"1.0.0","to_version":"1.1.0","source_ip":"127.0.0.1"}
```

Events: `server_started`, `server_stopped`, `rule_created`, `rule_updated`, `rule_deleted`, `rule_rollback`, `rule_executed` (sampled) and `sample_rate_changed`. `--audit-sample-rate` is a percentage of executions, default 10. Change it at runtime with `PUT /api/v1/config/audit-sample-rate` and `{"sample_rate": 50}`.

## Access

Ordo is meant for internal networks, so nothing is locked by default. What you can turn on:

- `--api-token <secret>`: data routes need `Authorization: Bearer <secret>` or `X-API-Token: <secret>`.
- `--cors-allowed-origins`: allow a browser app (such as Studio) on another origin.
- `--grpc-tls-cert` and `--grpc-tls-key` for gRPC TLS, `--grpc-mtls-enabled` for client certificates. For HTTPS, put a proxy in front.
- `--max-concurrent-executions`, `--request-timeout-secs`, `--max-request-body-bytes` to limit load.

## Tenants

With `--multi-tenancy-enabled`, every request names a tenant in the `X-Tenant-ID` header (gRPC: `x-tenant-id` metadata). Each tenant has its own rules, stored under `<rules-dir>/tenants/<id>/`, and its own limits:

```bash
curl -X POST http://localhost:8080/api/v1/tenants -H 'Content-Type: application/json' \
  -d '{"id":"shop-eu","name":"Shop EU","qps_limit":500,"burst_limit":1000,"execution_timeout_ms":50}'
```

Requests without the header use `--default-tenant` (default `default`).

## Next

- [Deployment](./deployment): several instances, Kubernetes, Nomad, signed rules
- [Embedding](./embed): run the engine inside a Rust program or a browser

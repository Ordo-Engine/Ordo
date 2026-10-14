# 运行服务

`ordo-server` 为业务系统执行规则。HTTP 默认在 8080 端口，gRPC 在 50051，也可以监听 Unix Socket。

## 启动

用 Docker：

```bash
docker run -p 8080:8080 -p 50051:50051 ghcr.io/ordo-engine/ordo:latest
curl http://localhost:8080/health
```

从源码构建（需要 Rust 1.83 或更高）：

```bash
git clone https://github.com/Ordo-Engine/Ordo.git && cd Ordo
cargo build --release
./target/release/ordo-server --rules-dir ./rules
```

每个参数都有对应的 `ORDO_*` 环境变量，见[服务器选项](/zh/reference/server-options)。

## 规则存到磁盘

不加 `--rules-dir` 时，规则只在内存里，重启就没了。加上之后：

- 启动时加载目录里的 `.json`、`.yaml`、`.yml` 文件，文件名就是规则名。
- 通过 API 创建、更新、删除规则时，会写回这个目录。
- `<rules-dir>/wal/` 里的预写日志保证写入在崩溃后不丢。

```bash
docker run -p 8080:8080 -v "$PWD/rules:/data/rules" --user "$(id -u):$(id -g)" \
  ghcr.io/ordo-engine/ordo:latest --rules-dir /data/rules
```

容器里要能写这个目录。在 Linux 上，`--user` 让容器以你的身份运行，才能写你的目录。

磁盘上的文件被 git 或部署脚本改了之后要自动加载，加 `--watch-rules`。也可以手动调用 `POST /api/v1/admin/reload`。

## 调用规则

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

加上 `"trace": true` 可以看到执行路径。一次跑多条输入用 `POST /api/v1/execute/discount/batch`，请求体是 `{"inputs": [...]}`。完整接口见 [HTTP API](/zh/api/http-api) 和 [gRPC API](/zh/api/grpc-api)。

Go、Python、Java 的客户端库在仓库的 [`sdk/`](https://github.com/Ordo-Engine/Ordo/tree/main/sdk) 目录，还没有发布到各自的包仓库。

## 版本与回滚

有 `--rules-dir` 时，每次更新都会把上一个版本留在规则文件旁边（`discount.v1.json`、`discount.v2.json` ……）。`--max-versions`（默认 10）决定保留几个。

```bash
curl http://localhost:8080/api/v1/rulesets/discount/versions
curl -X POST http://localhost:8080/api/v1/rulesets/discount/rollback \
  -H 'Content-Type: application/json' -d '{"seq": 1}'
```

```json
{ "status": "rolled_back", "name": "discount", "from_version": "1.1.0", "to_version": "1.0.0" }
```

没有 `--rules-dir` 时没有历史，回滚会返回错误。

## 健康检查

| 端点             | 用途                                                     |
| ---------------- | -------------------------------------------------------- |
| `/healthz/live`  | 存活检查：进程在就返回 `200`                             |
| `/healthz/ready` | 就绪检查：检查规则存储，以及 `--rules-dir` 是否可写      |
| `/health`        | 和 `/healthz/ready` 相同                                 |
| `/metrics`       | Prometheus 指标，见[指标](/zh/reference/metrics)         |

这些端点不受 API token 和并发上限限制。

## 审计日志

规则变更和部分执行记录会以 JSON 行写到 stdout；设置 `--audit-dir` 后还会按天写入文件：

```bash
ordo-server --rules-dir ./rules --audit-dir ./audit --audit-sample-rate 10
```

```json
{"timestamp":"…","level":"INFO","event":"rule_updated","rule_name":"discount","from_version":"1.0.0","to_version":"1.1.0","source_ip":"127.0.0.1"}
```

事件有 `server_started`、`server_stopped`、`rule_created`、`rule_updated`、`rule_deleted`、`rule_rollback`、`rule_executed`（抽样）和 `sample_rate_changed`。`--audit-sample-rate` 是记录执行的百分比，默认 10。运行时可以用 `PUT /api/v1/config/audit-sample-rate` 加 `{"sample_rate": 50}` 修改。

## 访问控制

Ordo 面向内网，所以默认什么都不锁。可以开启的有：

- `--api-token <secret>`：数据接口需要带 `Authorization: Bearer <secret>` 或 `X-API-Token: <secret>`。
- `--cors-allowed-origins`：允许其他域名的浏览器应用（比如 Studio）访问。
- gRPC TLS 用 `--grpc-tls-cert` 和 `--grpc-tls-key`，要客户端证书再加 `--grpc-mtls-enabled`。HTTPS 请在前面放一个代理。
- `--max-concurrent-executions`、`--request-timeout-secs`、`--max-request-body-bytes` 限制负载。

## 多租户

开启 `--multi-tenancy-enabled` 后，每个请求在 `X-Tenant-ID` 请求头里指定租户（gRPC 用 `x-tenant-id` 元数据）。每个租户有自己的规则，存在 `<rules-dir>/tenants/<id>/`，也有自己的限额：

```bash
curl -X POST http://localhost:8080/api/v1/tenants -H 'Content-Type: application/json' \
  -d '{"id":"shop-eu","name":"Shop EU","qps_limit":500,"burst_limit":1000,"execution_timeout_ms":50}'
```

不带请求头的请求使用 `--default-tenant`（默认 `default`）。

## 接下来

- [部署](./deployment)：多实例、Kubernetes、Nomad、签名规则
- [嵌入使用](./embed)：在 Rust 程序或浏览器里运行引擎

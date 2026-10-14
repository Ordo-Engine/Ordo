# 部署

对大多数团队来说，一个 `ordo-server` 加上持久磁盘上的 `--rules-dir` 就够了。这一页讲不够时还能加什么。

## 多实例

Ordo 用一个写节点加任意多个读节点。写节点接受规则变更。读节点处理查询和执行，所有写请求都回 `409 Conflict`：

```json
{ "error": "read_only", "message": "This instance is a read-only reader. Mutations must be sent to the writer.", "writer": "http://writer:8080" }
```

gRPC 下读节点返回 `FAILED_PRECONDITION`。读节点拿到变更有两种方式。

共享目录，适合多个实例共用一块磁盘（同一台机器或网络挂载）：

```bash
ordo-server --role writer --rules-dir /shared/rules --watch-rules
ordo-server --role reader --rules-dir /shared/rules --watch-rules \
  --writer-addr http://writer:8080 --port 8081
```

文件变了之后大约 200 毫秒内重新加载。拿不到文件系统事件时，每 30 秒轮询一次。

NATS JetStream，适合跨机器：

```bash
# 写节点
ordo-server --role writer --rules-dir /data/rules \
  --nats-url nats://nats:4222 --instance-id writer-1

# 每个读节点
ordo-server --role reader --writer-addr http://writer:8080 \
  --nats-url nats://nats:4222 --instance-id reader-1
```

写节点发布每一次变更。每个读节点有一个持久消费者，重启后从上次停下的地方接着收。NATS 断开时，每个实例继续用手上已有的规则服务。每个实例要有自己的 `--instance-id`（在 Kubernetes 里用 Pod 名）。

Docker 镜像已经带了 NATS 支持。从源码构建时要加上：`cargo build --release -p ordo-server --features nats-sync`。

规则执行不经过同步这条路径，所以读节点和单实例一样快。

## Kubernetes

仓库里的 [`deploy/k8s`](https://github.com/Ordo-Engine/Ordo/tree/main/deploy/k8s) 有可直接用的清单，包括服务器、平台、NATS 和 Postgres：

```bash
# 先把 Secret 文件里的 CHANGE_ME 换掉
kubectl apply -k deploy/k8s
kubectl -n ordo get pods -w
```

自己写清单的话，要点是这些：

```yaml
containers:
  - name: ordo-server
    image: ghcr.io/ordo-engine/ordo:latest
    args: ["--rules-dir=/data/rules"]
    ports:
      - { containerPort: 8080, name: http }
      - { containerPort: 50051, name: grpc }
    livenessProbe:
      httpGet: { path: /healthz/live, port: http }
    readinessProbe:
      httpGet: { path: /healthz/ready, port: http }
    volumeMounts:
      - { name: rules, mountPath: /data/rules }
```

规则在每个实例的内存里，从 `--rules-dir` 加载。副本多于一个时，要么用写节点、读节点加 NATS，要么让每个副本拿到同一份规则文件并开启 `--watch-rules`。否则在一个 Pod 上通过 API 建的规则，其他 Pod 上没有。

关闭时，服务最多等待 `--shutdown-timeout-secs`（默认 30 秒）让正在处理的请求完成。

## Nomad

Job 文件在 [`deploy/nomad`](https://github.com/Ordo-Engine/Ordo/tree/main/deploy/nomad)，带一个脚本：

```bash
./deploy/nomad/deploy.sh deploy dev    # 动态端口
./deploy/nomad/deploy.sh deploy prod   # 8080 和 50051 端口
```

## 签名规则

要确保只加载你签过名的规则，就用 Ed25519 签名并开启校验。下面三个工具由 `cargo build --release` 构建，和 `ordo-server` 在同一个目录。

```bash
ordo-keygen --output ./keys                          # keys/public.key、keys/private.key
ordo-sign --key ./keys/private.key --input rule.json # 生成 rule.signed.json
ordo-verify --key ./keys/public.key --input rule.signed.json
```

`ordo-sign` 会给 JSON 和 YAML 文件加上 `_signature` 字段，也能签编译好的 `.ordo` 文件。用 `--output` 指定输出文件名。不要把 `private.key` 放进仓库。

```bash
ordo-server --rules-dir ./rules \
  --signature-enabled --signature-require \
  --signature-trusted-keys-file /etc/ordo/trusted_keys.txt
```

| 参数 | 作用 |
| --- | --- |
| `--signature-enabled` | 对带签名的规则做校验 |
| `--signature-require` | 拒绝没有签名的规则，包括 API 上传的和本地文件 |
| `--signature-trusted-keys` | 信任的公钥，base64，逗号分隔 |
| `--signature-trusted-keys-file` | 同上，每行一个 |

只开 `--signature-enabled` 时，没签名的本地文件仍然会加载（服务会打一条警告）。加上 `--signature-require` 才会拒绝。设置 `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL=false` 会拒绝没签名的本地文件，但仍接受 API 上传的未签名规则。

通过 HTTP 上传时，签名可以放在请求体的 `_signature` 字段，也可以放在 `X-Ordo-Signature` 和 `X-Ordo-Public-Key` 请求头里。校验不通过返回 `403`。

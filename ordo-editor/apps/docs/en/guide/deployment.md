# Deployment

One `ordo-server` with `--rules-dir` on a persistent disk is enough for most teams. This page covers what to add when that is not enough.

## Several instances

Ordo uses one writer and any number of readers. The writer accepts rule changes. Readers serve reads and executions, and answer every write with `409 Conflict`:

```json
{ "error": "read_only", "message": "This instance is a read-only reader. Mutations must be sent to the writer.", "writer": "http://writer:8080" }
```

Over gRPC a reader returns `FAILED_PRECONDITION`. Readers get changes in one of two ways.

Shared folder, when instances share a disk (same machine or a network mount):

```bash
ordo-server --role writer --rules-dir /shared/rules --watch-rules
ordo-server --role reader --rules-dir /shared/rules --watch-rules \
  --writer-addr http://writer:8080 --port 8081
```

The watcher reloads a changed file within about 200 ms. If file events are not available, it polls every 30 seconds.

NATS JetStream, across machines:

```bash
# writer
ordo-server --role writer --rules-dir /data/rules \
  --nats-url nats://nats:4222 --instance-id writer-1

# each reader
ordo-server --role reader --writer-addr http://writer:8080 \
  --nats-url nats://nats:4222 --instance-id reader-1
```

The writer publishes each change. Every reader has a durable consumer, so a restarted reader catches up from where it stopped. If NATS goes away, every instance keeps serving the rules it has. Give each instance its own `--instance-id` (in Kubernetes, the pod name).

The Docker image includes NATS support. When building from source, add it: `cargo build --release -p ordo-server --features nats-sync`.

Rule execution does not touch the sync path, so readers run as fast as a single instance.

## Kubernetes

The repository has working manifests for the server, the platform, NATS and Postgres in [`deploy/k8s`](https://github.com/Ordo-Engine/Ordo/tree/main/deploy/k8s):

```bash
# replace the CHANGE_ME values in the Secret files first
kubectl apply -k deploy/k8s
kubectl -n ordo get pods -w
```

If you write your own, the parts that matter:

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

Rules live in each instance's memory, loaded from `--rules-dir`. With more than one replica, use the writer and reader roles with NATS, or give every replica the same rule files and `--watch-rules`. Otherwise a rule created through the API on one pod does not exist on the others.

On shutdown the server waits up to `--shutdown-timeout-secs` (default 30) for requests in flight.

## Nomad

Job files are in [`deploy/nomad`](https://github.com/Ordo-Engine/Ordo/tree/main/deploy/nomad), with a script:

```bash
./deploy/nomad/deploy.sh deploy dev    # dynamic ports
./deploy/nomad/deploy.sh deploy prod   # ports 8080 and 50051
```

## Signed rules

To make sure only rules you signed are loaded, sign them with Ed25519 and turn on verification. The three tools below are built by `cargo build --release`, next to `ordo-server`.

```bash
ordo-keygen --output ./keys                          # keys/public.key, keys/private.key
ordo-sign --key ./keys/private.key --input rule.json # writes rule.signed.json
ordo-verify --key ./keys/public.key --input rule.signed.json
```

`ordo-sign` adds a `_signature` field to JSON and YAML files, and also signs compiled `.ordo` files. Use `--output` to choose the file name. Keep `private.key` out of the repository.

```bash
ordo-server --rules-dir ./rules \
  --signature-enabled --signature-require \
  --signature-trusted-keys-file /etc/ordo/trusted_keys.txt
```

| Flag | Effect |
| --- | --- |
| `--signature-enabled` | Check signatures on rules that have one |
| `--signature-require` | Reject unsigned rules, from the API and from local files |
| `--signature-trusted-keys` | Trusted public keys, base64, comma-separated |
| `--signature-trusted-keys-file` | The same, one key per line |

With `--signature-enabled` alone, unsigned local files still load (the server logs a warning). Add `--signature-require` to reject them. Setting `ORDO_SIGNATURE_ALLOW_UNSIGNED_LOCAL=false` rejects unsigned local files while still accepting unsigned rules from the API.

When uploading over HTTP, the signature can be in the body as `_signature`, or in the `X-Ordo-Signature` and `X-Ordo-Public-Key` headers. A rule that fails verification gets `403`.

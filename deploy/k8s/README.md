# Ordo on Kubernetes

Working manifests for the full Ordo stack. They mirror the env/config in
`compose.yml` and the Nomad jobs under `deploy/nomad/`, so behaviour is
consistent across orchestrators.

## Topology

```
                    Ingress / LB
                         │
                         ▼
                 ordo-platform  ──proxy /engine──▶  ordo-server
                 (gateway :3001)                    (engine :8080 / gRPC :50051)
                   │      ▲                              │      ▲
        Postgres ◀─┘      │ release/sync events          │      │ rule sync
        (state)           │  (NATS JetStream)            └──────┘
                          │                                  │
                 ordo-platform-worker ───────────  NATS JetStream (stream "ordo-rules")
```

| Component              | Kind        | Exposed | Persistence                         |
|------------------------|-------------|---------|-------------------------------------|
| `ordo-server`          | Deployment  | internal ClusterIP (8080/50051) | PVC `ordo-server-rules` (rules + WAL) |
| `ordo-platform`        | Deployment  | ClusterIP 3001 (front with Ingress) | PostgreSQL                |
| `ordo-platform-worker` | Deployment  | none    | PostgreSQL + NATS                   |
| `ordo-postgres`        | StatefulSet | internal | PVC (volumeClaimTemplate)          |
| `ordo-nats`            | StatefulSet | internal | PVC (JetStream file store)         |

## Probes

| Service        | Liveness          | Readiness          | Notes |
|----------------|-------------------|--------------------|-------|
| `ordo-server`  | `GET /healthz/live` | `GET /healthz/ready` | real engine endpoints |
| `ordo-platform`| `GET /health`     | `GET /health`      | currently a 200 "ok" stub; deep readiness is being hardened separately |
| `ordo-postgres`| `pg_isready`      | `pg_isready`       | |
| `ordo-nats`    | `GET :8222/healthz` | `GET :8222/healthz` | |

## Quick start

```bash
# 1. Edit the placeholder secrets first! (see below)
#    deploy/k8s/server/secret.yaml
#    deploy/k8s/platform/secret.yaml
#    deploy/k8s/dependencies/postgres.yaml

# 2. Apply everything
kubectl apply -k deploy/k8s

# 3. Watch it come up
kubectl -n ordo get pods -w

# 4. Reach the platform locally
kubectl -n ordo port-forward svc/ordo-platform 3001:3001
```

## Before production

1. **Secrets** — every `Secret` here ships placeholder values that scream
   `CHANGE_ME`. Replace them, ideally via `kubectl create secret`,
   sealed-secrets, external-secrets, or Vault — do not commit real secrets.
   `ORDO_JWT_SECRET` must be a random value of at least 32 characters.
2. **CORS** — `ORDO_PLATFORM_CORS_ORIGINS` defaults to your Studio origin, not
   `*`. Keep it scoped to real origins.
3. **Postgres** — the bundled single-node StatefulSet has no HA and no backups.
   Prefer a managed Postgres (RDS / Cloud SQL / CloudNativePG). To bring your
   own, drop `dependencies/postgres.yaml` from `kustomization.yaml` and point
   `ORDO_DATABASE_URL` (in `platform/secret.yaml`) at it.
4. **NATS** — for production run a 3-node JetStream cluster (NATS Helm chart),
   then drop `dependencies/nats.yaml` and repoint `ORDO_NATS_URL`.
5. **Images** — replace `ghcr.io/pama-lee/ordo-*:latest` with pinned tags.
6. **Backups / DR** — see `deploy/BACKUP_AND_DR.md`.

## Config reference

Every `ORDO_*` variable (name, default, purpose) is documented in
[`deploy/CONFIGURATION.md`](../CONFIGURATION.md).

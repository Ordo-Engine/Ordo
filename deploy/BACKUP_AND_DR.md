# Ordo Backup & Disaster Recovery Runbook

This runbook covers backup and recovery for the two stateful stores behind the
Ordo control plane (`ordo-platform`):

1. **PostgreSQL** — the system of record for all control-plane data: users,
   organizations, projects, members/RBAC, rulesets and drafts, release requests,
   and the server registry. `ordo-platform` runs its embedded `sqlx` migrations
   automatically on startup, so the schema is re-created on a fresh DB; backups
   exist to recover *data*, not schema.
2. **NATS JetStream** stream **`ordo-rules`** (subjects `ordo.rules.>`) — the
   durable rule-sync + release-event log. It is the source of truth for
   **in-flight releases** propagating from the platform to `ordo-server`
   instances. Losing it can strand a release mid-flight even if Postgres is intact.

> The rule engine (`ordo-server`) keeps its rules under `ORDO_RULES_DIR` + a WAL.
> In a platform-managed deployment those rules are derived state (re-synced from
> the platform via JetStream), so the engine PVC is **not** the primary backup
> target — Postgres + JetStream are. If you run `ordo-server` standalone (no
> platform), back up its `ORDO_RULES_DIR` volume instead, since it is then the
> only copy.

---

## Recovery objectives (recommended targets)

These are sane defaults for an internal infrastructure service; tune to your SLA.

| Store              | RPO (max data loss) | RTO (max downtime) | How achieved |
|--------------------|---------------------|--------------------|--------------|
| PostgreSQL         | ≤ 5 min             | ≤ 30 min           | Continuous WAL archiving / PITR (managed PG), or ≥ hourly `pg_dump` |
| NATS JetStream     | ≤ 15 min            | ≤ 30 min           | Periodic `nats stream backup` + file storage on durable PVC |
| Full platform      | ≤ 15 min            | ≤ 1 h              | Restore PG first, then JetStream, then start services |

- **RPO** is bounded by your backup interval. Scheduled `pg_dump`/`nats stream
  backup` (e.g. via CronJob) gives an RPO equal to the schedule. For a tighter
  Postgres RPO, prefer Point-In-Time Recovery (below).
- Keep at least one **off-cluster / off-host** copy (object storage, different
  region). A backup on the same PVC/node as the database is not DR.

---

## What to back up

| Component | Mechanism | Tool |
|-----------|-----------|------|
| PostgreSQL (logical) | `pg_dump` custom-format dump | `scripts/backup.sh` |
| PostgreSQL (PITR)    | base backup + WAL archive | managed PG / `pgBackRest` |
| JetStream `ordo-rules` | stream backup (messages + config) | `scripts/backup.sh` |

---

## Quick start — scripted logical backup

`scripts/backup.sh` captures both stores into one timestamped directory.

```bash
export ORDO_DATABASE_URL="postgresql://ordo:PASSWORD@localhost:5432/ordo_platform"
export ORDO_NATS_URL="nats://localhost:4222"
# Optional overrides:
#   ORDO_NATS_STREAM (default: ordo-rules)
#   BACKUP_DIR       (default: ./backups)

./scripts/backup.sh
# -> ./backups/ordo-backup-<UTC>/{platform.dump, jetstream/, MANIFEST.txt}
```

Requirements: `pg_dump`/`pg_restore` (postgresql-client) and the
[`nats` CLI](https://github.com/nats-io/natscli). Skip a store with `--no-pg`
or `--no-nats`.

**Always push the resulting directory off-box**, e.g.:

```bash
tar -czf ordo-backup-<UTC>.tgz -C ./backups ordo-backup-<UTC>
aws s3 cp ordo-backup-<UTC>.tgz s3://your-bucket/ordo/   # or rclone, gsutil, etc.
```

### In-cluster (Kubernetes)

Run the script from a pod that can reach Postgres and NATS, e.g. exec into the
platform pod (it has `curl` but not `pg_dump`/`nats`) or schedule a small
CronJob with a `postgres:16-alpine` + `natscli` image that mounts a backup
volume and uploads to object storage. Point `ORDO_DATABASE_URL` at
`ordo-postgres.ordo.svc.cluster.local:5432` and `ORDO_NATS_URL` at
`nats://ordo-nats.ordo.svc.cluster.local:4222`.

### Docker Compose

```bash
# Postgres dump straight from the compose service:
docker compose exec -T postgres \
  pg_dump -U ordo -Fc ordo_platform > platform.dump

# JetStream (needs the nats CLI pointed at the published/forwarded port):
nats --server nats://localhost:4222 stream backup ordo-rules ./jetstream
```

---

## PostgreSQL Point-In-Time Recovery (preferred for production)

Logical `pg_dump` is simple and portable but has an RPO equal to the dump
interval. For a tight RPO use PITR:

- **Managed Postgres** (RDS, Cloud SQL, AlloyDB, CloudNativePG): enable automated
  backups + WAL archiving and set the retention window. Restore = "create new
  instance from point in time", then repoint `ORDO_DATABASE_URL`.
- **Self-managed**: use `pgBackRest` or `wal-g` for a base backup + continuous
  WAL archive to object storage. Recover with `restore_command` /
  `recovery_target_time`.

PITR covers Postgres only — still back up JetStream separately.

---

## Recovery procedure (full platform)

Restore order matters: **Postgres first, then JetStream, then start services.**

1. **Stop writers.** Scale down the platform and worker so nothing mutates state
   mid-restore:
   ```bash
   kubectl -n ordo scale deploy/ordo-platform deploy/ordo-platform-worker --replicas=0
   # compose: docker compose stop ordo-platform ordo-platform-worker
   ```

2. **Restore PostgreSQL.**
   - PITR: provision/restore the instance to the target time, then set
     `ORDO_DATABASE_URL` to it.
   - Logical dump:
     ```bash
     export ORDO_DATABASE_URL="postgresql://ordo:PASSWORD@HOST:5432/ordo_platform"
     ./scripts/restore.sh --no-nats ./backups/ordo-backup-<UTC>
     ```
     (`restore.sh` uses `pg_restore --clean --if-exists`, dropping existing
     objects first. The target database must already exist — create it with
     `createdb` if restoring into a fresh server.)

3. **Restore JetStream.**
   ```bash
   export ORDO_NATS_URL="nats://HOST:4222"
   ./scripts/restore.sh --no-pg ./backups/ordo-backup-<UTC>
   ```
   `nats stream restore` recreates the `ordo-rules` stream with its messages and
   config. If the stream still exists, delete it first
   (`nats stream rm ordo-rules`) or restore into a clean NATS.

   > Both steps in one shot (Postgres + JetStream): `./scripts/restore.sh ./backups/ordo-backup-<UTC>`

4. **Bring services back up.**
   ```bash
   kubectl -n ordo scale deploy/ordo-platform deploy/ordo-platform-worker --replicas=2
   kubectl -n ordo rollout status deploy/ordo-platform
   ```
   On startup the platform re-applies migrations (no-ops on a restored schema)
   and reconnects to NATS; `ordo-server` instances re-sync rules from the
   restored `ordo-rules` stream.

5. **Verify.**
   - `GET /health` on the platform returns `200`.
   - Sign in and confirm orgs/projects/rulesets are present.
   - Confirm `ordo-server` readiness: `GET /healthz/ready` is `200`.
   - Spot-check that a known ruleset executes as expected.
   - Re-drive any release that was in-flight at backup time, since events after
     the backup point are lost.

---

## Recovery scenarios

| Scenario | Action |
|----------|--------|
| Postgres data corruption / bad migration | Restore PG (PITR to just before, or last dump). JetStream usually intact — skip with `--no-pg`/`--no-nats` as needed. |
| JetStream volume lost | Restore the `ordo-rules` stream from the latest stream backup; re-drive in-flight releases. |
| Whole namespace/cluster lost | Re-apply `deploy/k8s` (with real Secrets), then run the full recovery procedure. |
| Accidental ruleset/release deletion | PITR Postgres to just before the deletion into a scratch DB, export the affected rows, re-apply. |

---

## Backup hygiene checklist

- [ ] Backups run on a schedule (CronJob / cron) — not manually.
- [ ] At least one copy is **off-cluster / off-region**.
- [ ] Backups are **encrypted at rest** and access is restricted (they contain
      tenant data; the DB also stores secrets/credentials).
- [ ] Retention is defined (e.g. 7 daily + 4 weekly) and old backups are pruned.
- [ ] A **restore drill** is performed periodically into a scratch environment —
      an untested backup is not a backup.
- [ ] Postgres and JetStream backups are taken close together in time to
      minimize cross-store skew during recovery.

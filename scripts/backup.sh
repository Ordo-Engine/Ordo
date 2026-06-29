#!/usr/bin/env bash
#
# backup.sh — back up Ordo platform state: PostgreSQL + NATS JetStream.
#
# Captures the two stateful stores behind ordo-platform:
#   1. PostgreSQL  — all control-plane data (orgs, projects, rulesets, releases,
#                    RBAC, server registry). Dumped with pg_dump custom format.
#   2. NATS JetStream stream "ordo-rules" — the rule-sync + release-event log,
#                    which is the source of truth for in-flight releases.
#
# Output layout:
#   $BACKUP_DIR/ordo-backup-<UTC timestamp>/
#     ├── platform.dump        (pg_dump -Fc; restore with pg_restore)
#     ├── jetstream/           (nats stream backup output)
#     └── MANIFEST.txt         (what/when/versions)
#
# Configuration (environment variables):
#   ORDO_DATABASE_URL   PostgreSQL connection URL. Required unless --no-pg.
#                       e.g. postgresql://ordo:pass@localhost:5432/ordo_platform
#   ORDO_NATS_URL       NATS server URL.            Default: nats://localhost:4222
#   ORDO_NATS_STREAM    JetStream stream to back up. Default: ordo-rules
#   BACKUP_DIR          Destination root directory.  Default: ./backups
#
# Flags:
#   --no-pg     Skip the PostgreSQL dump.
#   --no-nats   Skip the JetStream backup.
#   -h|--help   Show this help.
#
# Requires: pg_dump (postgresql-client), nats (NATS CLI). See deploy/BACKUP_AND_DR.md.
#
# Example:
#   ORDO_DATABASE_URL=postgresql://ordo:pass@localhost:5432/ordo_platform \
#   ORDO_NATS_URL=nats://localhost:4222 \
#   ./scripts/backup.sh
#
set -euo pipefail

NATS_URL="${ORDO_NATS_URL:-nats://localhost:4222}"
NATS_STREAM="${ORDO_NATS_STREAM:-ordo-rules}"
BACKUP_DIR="${BACKUP_DIR:-./backups}"
DO_PG=1
DO_NATS=1

usage() { sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; s/^#//'; exit "${1:-0}"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --no-pg)   DO_PG=0 ;;
    --no-nats) DO_NATS=0 ;;
    -h|--help) usage 0 ;;
    *) echo "error: unknown argument '$1'" >&2; usage 1 ;;
  esac
  shift
done

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
dest="${BACKUP_DIR%/}/ordo-backup-${timestamp}"
mkdir -p "$dest"

echo "==> Ordo backup -> $dest"
{
  echo "Ordo backup"
  echo "created_utc: $timestamp"
  echo "host:        $(hostname 2>/dev/null || echo unknown)"
} > "$dest/MANIFEST.txt"

# ── PostgreSQL ────────────────────────────────────────────────────────────────
if [ "$DO_PG" -eq 1 ]; then
  if [ -z "${ORDO_DATABASE_URL:-}" ]; then
    echo "error: ORDO_DATABASE_URL is not set (use --no-pg to skip Postgres)" >&2
    exit 1
  fi
  if ! command -v pg_dump >/dev/null 2>&1; then
    echo "error: pg_dump not found — install the postgresql-client package" >&2
    exit 1
  fi
  echo "==> pg_dump (custom format) -> platform.dump"
  pg_dump --format=custom --no-owner --no-privileges \
    --file="$dest/platform.dump" "$ORDO_DATABASE_URL"
  echo "pg_dump:     platform.dump ($(pg_dump --version | head -n1))" >> "$dest/MANIFEST.txt"
else
  echo "==> skipping Postgres (--no-pg)"
fi

# ── NATS JetStream ────────────────────────────────────────────────────────────
if [ "$DO_NATS" -eq 1 ]; then
  if ! command -v nats >/dev/null 2>&1; then
    echo "error: nats CLI not found — install from https://github.com/nats-io/natscli" >&2
    exit 1
  fi
  echo "==> nats stream backup '$NATS_STREAM' -> jetstream/"
  nats --server "$NATS_URL" stream backup "$NATS_STREAM" "$dest/jetstream"
  echo "nats_stream: $NATS_STREAM ($(nats --version 2>&1 | head -n1))" >> "$dest/MANIFEST.txt"
else
  echo "==> skipping JetStream (--no-nats)"
fi

echo "==> Done. Backup at: $dest"
echo "    Restore with: ./scripts/restore.sh \"$dest\""

#!/usr/bin/env bash
#
# restore.sh — restore Ordo platform state from a backup.sh backup directory.
#
# !! DESTRUCTIVE !! This overwrites the target PostgreSQL database and replaces
# the NATS JetStream stream. Run only against the environment you intend to
# restore, ideally with the platform + worker STOPPED. See deploy/BACKUP_AND_DR.md.
#
# Usage:
#   ./scripts/restore.sh [flags] <backup-dir>
#
# <backup-dir> is a directory produced by scripts/backup.sh, e.g.
#   ./backups/ordo-backup-20260629T101500Z
#
# Configuration (environment variables):
#   ORDO_DATABASE_URL   Target PostgreSQL URL. Required unless --no-pg.
#   ORDO_NATS_URL       NATS server URL.        Default: nats://localhost:4222
#
# Flags:
#   --no-pg     Skip the PostgreSQL restore.
#   --no-nats   Skip the JetStream restore.
#   -f|--force  Do not prompt for confirmation.
#   -h|--help   Show this help.
#
# Requires: pg_restore (postgresql-client), nats (NATS CLI).
#
set -euo pipefail

NATS_URL="${ORDO_NATS_URL:-nats://localhost:4222}"
DO_PG=1
DO_NATS=1
FORCE=0
BACKUP_PATH=""

usage() { sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; s/^#//'; exit "${1:-0}"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --no-pg)    DO_PG=0 ;;
    --no-nats)  DO_NATS=0 ;;
    -f|--force) FORCE=1 ;;
    -h|--help)  usage 0 ;;
    -*)         echo "error: unknown flag '$1'" >&2; usage 1 ;;
    *)          BACKUP_PATH="$1" ;;
  esac
  shift
done

if [ -z "$BACKUP_PATH" ]; then
  echo "error: no <backup-dir> given" >&2
  usage 1
fi
if [ ! -d "$BACKUP_PATH" ]; then
  echo "error: backup directory not found: $BACKUP_PATH" >&2
  exit 1
fi

echo "==> Restore source: $BACKUP_PATH"
if [ "$FORCE" -ne 1 ]; then
  echo "!! This will OVERWRITE the target Postgres DB and JetStream stream."
  printf "Type 'yes' to continue: "
  read -r reply
  [ "$reply" = "yes" ] || { echo "aborted."; exit 1; }
fi

# ── PostgreSQL ────────────────────────────────────────────────────────────────
if [ "$DO_PG" -eq 1 ]; then
  dump="$BACKUP_PATH/platform.dump"
  if [ ! -f "$dump" ]; then
    echo "error: $dump not found (use --no-pg to skip)" >&2
    exit 1
  fi
  if [ -z "${ORDO_DATABASE_URL:-}" ]; then
    echo "error: ORDO_DATABASE_URL is not set (use --no-pg to skip Postgres)" >&2
    exit 1
  fi
  if ! command -v pg_restore >/dev/null 2>&1; then
    echo "error: pg_restore not found — install the postgresql-client package" >&2
    exit 1
  fi
  echo "==> pg_restore (--clean --if-exists) from platform.dump"
  # --clean/--if-exists drop existing objects first; --no-owner avoids role
  # mismatches across environments. Exit status is non-zero on harmless warnings,
  # so we let pg_restore print them without aborting the whole script.
  pg_restore --clean --if-exists --no-owner --no-privileges \
    --dbname="$ORDO_DATABASE_URL" "$dump" || \
    echo "   (pg_restore reported warnings; review output above)"
else
  echo "==> skipping Postgres (--no-pg)"
fi

# ── NATS JetStream ────────────────────────────────────────────────────────────
if [ "$DO_NATS" -eq 1 ]; then
  js="$BACKUP_PATH/jetstream"
  if [ ! -d "$js" ]; then
    echo "error: $js not found (use --no-nats to skip)" >&2
    exit 1
  fi
  if ! command -v nats >/dev/null 2>&1; then
    echo "error: nats CLI not found — install from https://github.com/nats-io/natscli" >&2
    exit 1
  fi
  echo "==> nats stream restore from jetstream/"
  nats --server "$NATS_URL" stream restore "$js"
else
  echo "==> skipping JetStream (--no-nats)"
fi

echo "==> Restore complete. Start ordo-platform / ordo-platform-worker and verify."

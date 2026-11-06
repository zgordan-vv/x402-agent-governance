#!/usr/bin/env sh
set -eu

: "${POSTGRES_DB:?Set POSTGRES_DB}"
: "${POSTGRES_USER:?Set POSTGRES_USER}"
: "${PGHOST:=127.0.0.1}"
: "${PGPORT:=5433}"
: "${BACKUP_DIR:=./backups}"

mkdir -p "$BACKUP_DIR"
stamp=$(date -u +%Y%m%dT%H%M%SZ)
target="$BACKUP_DIR/${POSTGRES_DB}-${stamp}.dump"
pg_dump --format=custom --no-owner --no-privileges --file="$target" --host="$PGHOST" --port="$PGPORT" --username="$POSTGRES_USER" "$POSTGRES_DB"
printf '%s\n' "$target"

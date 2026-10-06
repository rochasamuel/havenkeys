#!/bin/sh
# Restore a backup into the database. Replaces everything currently stored.
#   ./restore.sh backups/havenkeys-2026-10-06.dump
set -eu
cd "$(dirname "$0")"
[ $# -eq 1 ] || { echo "usage: ./restore.sh backups/havenkeys-YYYY-MM-DD.dump" >&2; ls backups 2>/dev/null; exit 2; }
[ -f "$1" ] || { echo "restore: $1 not found" >&2; exit 1; }
printf 'This replaces the server'"'"'s data with %s. Type yes to continue: ' "$1"
read -r answer
[ "$answer" = "yes" ] || { echo "Cancelled."; exit 1; }
docker compose stop server
docker compose exec -T db pg_restore -U havenkeys -d havenkeys --clean --if-exists --no-owner <"$1"
docker compose start server
echo "Restored. Open a HavenKeys app and check your items."

#!/bin/sh
# Restore a backup into the database. Replaces everything currently stored.
#   ./restore.sh backups/havenkeys-2026-10-06.dump
set -eu
# Resolve the dump path before changing directory.
case "${1:-}" in
/* | "") dump=${1:-} ;;
*) dump="$PWD/$1" ;;
esac
cd "$(dirname "$0")"
[ $# -eq 1 ] || { echo "usage: ./restore.sh backups/havenkeys-YYYY-MM-DD.dump" >&2; ls backups 2>/dev/null; exit 2; }
# Backups are written by the container as root with mode 600. A file the
# host user cannot read is streamed out through the backup container instead.
if [ -r "$dump" ]; then via_container=0
elif [ -e "$dump" ] && [ "$(dirname "$dump")" = "$PWD/backups" ]; then via_container=1
else echo "restore: $1 not found or not readable" >&2; exit 1; fi
printf 'This replaces the server'"'"'s data with %s. Type yes to continue: ' "$1"
read -r answer
[ "$answer" = "yes" ] || { echo "Cancelled."; exit 1; }
docker compose stop server
if [ "$via_container" = 1 ]; then
	docker compose exec -T backup cat "/backups/$(basename "$dump")"
else
	cat "$dump"
fi | docker compose exec -T db pg_restore -U havenkeys -d havenkeys --clean --if-exists --no-owner || {
	echo "restore failed; the server is stopped — fix the problem, then run: docker compose start server" >&2
	exit 1
}
docker compose start server
echo "Restored. Open a HavenKeys app and check your items."

#!/bin/sh
# Local smoke test: builds the server image from this checkout, starts the
# bundle on localhost with Caddy's internal CA, checks health, takes a backup.
# Usage (repo root): sh deploy/compose/test/smoke.sh
set -eu
# Every compose call uses an explicit project name so this test can never
# touch a live install (the bundle's default project is "havenkeys").
export COMPOSE_PROJECT_NAME=havenkeys-smoke
dc() { docker compose -p havenkeys-smoke "$@"; }
root=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d)
trap 'cd "$work" && dc down -v >/dev/null 2>&1; rm -rf "$work"' EXIT
docker build -t havenkeys-server:smoke "$root"
cp "$root"/deploy/compose/compose.yaml "$root"/deploy/compose/Caddyfile "$root"/deploy/compose/backup.sh "$root"/deploy/compose/restore.sh "$work"/
cd "$work"
mkdir -m 700 backups
cat >.env <<EOT
HAVENKEYS_DOMAIN=localhost
ACME_EMAIL=smoke@example.com
SERVER_SECRET=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
POSTGRES_PASSWORD=smoke$(head -c 12 /dev/urandom | base64 | tr -d '/+=\n')
HAVENKEYS_IMAGE=havenkeys-server
HAVENKEYS_VERSION=smoke
HTTP_PORT=18080
HTTPS_PORT=18443
EOT
dc config -q
dc up -d
i=0
until curl -fsSk https://localhost:18443/v1/health; do
	i=$((i + 1)); [ $i -lt 30 ] || { dc logs; exit 1; }; sleep 2
done
echo
dc exec -T backup /bin/sh /usr/local/bin/havenkeys-backup now
ls backups | grep -q '^havenkeys-.*\.dump$'
good=$(cd backups && ls havenkeys-*.dump | head -n1)
before=$(sha256sum "backups/$good")
# A failing database must not replace or delete the good dump.
dc stop db
if dc exec -T backup /bin/sh /usr/local/bin/havenkeys-backup now; then
	echo "smoke: backup should have failed with the database stopped" >&2; exit 1
fi
[ -z "$(ls -A backups | grep partial || true)" ] || { echo "smoke: .partial left behind" >&2; exit 1; }
[ "$(sha256sum "backups/$good")" = "$before" ] || { echo "smoke: good dump changed" >&2; exit 1; }
dc start db
# Restore the good dump end to end.
printf 'yes\n' | sh ./restore.sh "backups/$good"
i=0
until curl -fsSk https://localhost:18443/v1/health >/dev/null; do
	i=$((i + 1)); [ $i -lt 30 ] || { dc logs; exit 1; }; sleep 2
done
dc exec -T server havenkeys-server admin list-accounts
echo "smoke: OK"

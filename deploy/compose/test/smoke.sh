#!/bin/sh
# Local smoke test: builds the server image from this checkout, starts the
# bundle on localhost with Caddy's internal CA, checks health, takes a backup.
# Usage (repo root): sh deploy/compose/test/smoke.sh
set -eu
root=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d)
trap 'cd "$work" && docker compose down -v >/dev/null 2>&1; rm -rf "$work"' EXIT
docker build -t havenkeys-server:smoke "$root"
cp "$root"/deploy/compose/compose.yaml "$root"/deploy/compose/Caddyfile "$root"/deploy/compose/backup.sh "$work"/
cd "$work"
mkdir backups
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
docker compose config -q
docker compose up -d
i=0
until curl -fsSk https://localhost:18443/v1/health; do
	i=$((i + 1)); [ $i -lt 30 ] || { docker compose logs; exit 1; }; sleep 2
done
echo
docker compose exec -T backup /bin/sh /usr/local/bin/havenkeys-backup now
ls backups | grep -q '^havenkeys-.*\.dump$'
good=$(cd backups && ls havenkeys-*.dump | head -n1)
sum() { docker compose exec -T backup sha256sum "/backups/$good"; }
before=$(sum)
# A failing database must not replace or delete the good dump.
docker compose stop db
if docker compose exec -T backup /bin/sh /usr/local/bin/havenkeys-backup now; then
	echo "smoke: backup should have failed with the database stopped" >&2; exit 1
fi
[ -z "$(ls -A backups | grep partial || true)" ] || { echo "smoke: .partial left behind" >&2; exit 1; }
[ "$(sum)" = "$before" ] || { echo "smoke: good dump changed" >&2; exit 1; }
docker compose start db
docker compose exec -T server havenkeys-server admin list-accounts
echo "smoke: OK"

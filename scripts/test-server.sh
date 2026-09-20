#!/usr/bin/env bash
# Run the havenkeys-server test suite against a disposable Postgres.
#
# The suite creates one database per test and drops it afterwards, so the
# container can be reused between runs and thrown away at any time:
#
#   docker rm -f havenkeys-test-pg
#
# Usage: scripts/test-server.sh [extra cargo test arguments]
set -euo pipefail

CONTAINER=havenkeys-test-pg
PORT="${HAVENKEYS_TEST_PG_PORT:-5433}"
export HAVENKEYS_TEST_DATABASE_URL="${HAVENKEYS_TEST_DATABASE_URL:-postgres://postgres:postgres@localhost:${PORT}/postgres?sslmode=disable}"

if ! docker ps --format '{{.Names}}' | grep -qx "$CONTAINER"; then
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  echo "starting $CONTAINER on port $PORT"
  docker run -d --name "$CONTAINER" \
    -p "${PORT}:5432" \
    -e POSTGRES_PASSWORD=postgres \
    -e POSTGRES_USER=postgres \
    -e POSTGRES_DB=postgres \
    postgres:17-alpine >/dev/null
fi

echo -n "waiting for Postgres"
for _ in $(seq 1 60); do
  if docker exec "$CONTAINER" pg_isready -q -U postgres; then
    echo " ok"
    break
  fi
  echo -n "."
  sleep 1
done

cargo test -p havenkeys-server "$@"

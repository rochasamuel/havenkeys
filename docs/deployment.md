# Deploying `havenkeys-server`

> This software has not undergone an independent security audit.

The server is the authority for a vault's contents: the local SQLite on each
device is a replica that follows it, including deletions
(`docs/server-sync.md`, design §9). **A tested backup is therefore a
prerequisite of storing a real vault, not a follow-up task.** §5 is the part
of this document that must not be skipped.

What the server can and cannot see is in `docs/server-sync.md`; in short, it
holds ciphertext it has no key for, plus the metadata needed to route it.

---

## 1. What you need

* A Postgres 14 or newer database (Railway's plugin, or any managed Postgres).
* A host that can run a container and terminate TLS. **Plain HTTP is not an
  option**: the auth key is a bearer proof of the master password and the
  Secret Key, and TLS is the only thing protecting it in transit.
* The `havenkeys-server` image, built from the repository root:

```sh
docker build -f crates/havenkeys-server/Dockerfile -t havenkeys-server .
```

## 2. Environment

| Variable | Required | Meaning |
|---|---|---|
| `DATABASE_URL` | yes | `postgres://user:pass@host:5432/db`. Add `?sslmode=disable` only on a private network you trust. |
| `SERVER_SECRET` | yes | 32 random bytes, base64. Used only to derive the decoy account id and salt that keep `auth/params` from revealing whether an email has an account. |
| `PORT` | no | Defaults to 8080. Railway sets it. |
| `HAVENKEYS_CORS_ORIGIN` | no | Exactly one browser origin. Leave unset: nothing in the MVP calls this API from a browser, and there is no wildcard. |
| `HAVENKEYS_TRUST_FORWARDED_FOR` | no | `1` only when a proxy you control sets `X-Forwarded-For`. Off by default, because a client could otherwise spread its login attempts over invented addresses and escape the per-IP rate limit. |

Generate the secret once and store it as a platform variable:

```sh
openssl rand -base64 32
```

Changing `SERVER_SECRET` later is harmless to vault data — it protects no
ciphertext — but it changes the decoy answers, which is visible to anyone
probing `auth/params`.

Migrations run at startup, so a fresh database needs no manual step.

## 3. Railway

1. Create a project, add the **Postgres** plugin, and add a service from this
   repository. `crates/havenkeys-server/railway.json` already points at the
   Dockerfile and sets `/v1/health` as the health check.
2. Set `SERVER_SECRET` on the service. `DATABASE_URL` comes from the plugin —
   use the **private** network URL if the plugin offers one.
3. Deploy, then confirm:

```sh
curl -si https://<your-host>/v1/health          # 200 {"status":"ok"}
curl -si http://<your-host>/v1/health           # must redirect or fail, never serve
```

4. Confirm TLS is real (not a self-signed placeholder):

```sh
curl -sS --proto '=https' --tlsv1.2 https://<your-host>/v1/health >/dev/null && echo "TLS ok"
```

## 4. The first account

There is no public signup. Accounts exist because an operator created one:

```sh
havenkeys-server admin new-account --email you@example.com \
                                   --server-url https://<your-host>
```

It prints one invite string (`HKINV1-…`), **once**. Only its SHA-256 is
stored, it is single-use, and it expires after seven days. Paste it into the
desktop app's first-run screen.

Other commands:

```sh
havenkeys-server admin list-accounts
havenkeys-server admin delete-account --email you@example.com   # irreversible
```

On Railway, run these in a one-off shell against the same service so they see
the same `DATABASE_URL`.

## 5. Backups, and the restore drill (blocking)

**Railway does not enable scheduled Postgres backups on every plan.** Check,
and turn them on, before the vault holds anything you would miss.

A backup that has never been restored is not a backup. Do this once, and
again after any change to the database plan:

```sh
# 1. Take a dump of the live database.
pg_dump --format=custom "$DATABASE_URL" > havenkeys-$(date +%F).dump

# 2. Restore it into a scratch database (locally is fine).
createdb havenkeys_restore_test
pg_restore --clean --if-exists --no-owner \
           --dbname postgres://postgres:postgres@localhost:5433/havenkeys_restore_test \
           havenkeys-$(date +%F).dump

# 3. Point a server at the restored copy and check it serves a real vault.
DATABASE_URL=postgres://postgres:postgres@localhost:5433/havenkeys_restore_test \
SERVER_SECRET=$(openssl rand -base64 32) \
PORT=8081 havenkeys-server &

curl -s localhost:8081/v1/health
psql "$RESTORE_URL" -c "SELECT count(*) FROM accounts;"
psql "$RESTORE_URL" -c "SELECT count(*) FROM items WHERE deleted_at IS NULL;"
```

4. Sign in from a device against the restored server and confirm the vault
   unlocks and the items are there. Only that proves the ciphertext, the
   header and the KDF parameters all survived — the row counts do not.

Keep the dump somewhere the server cannot reach. A server that is compromised
should not be able to destroy its own backups.

**Why this is not optional:** under the previous folder-sync design each
device held an independent authoritative copy, which was the real backup.
It is now a cache. A server that loses data, or is made to serve a deletion,
takes every device's copy with it — by construction, because the server is
the authority and a deletion it serves is indistinguishable from a real one.
The mitigation is backups and the user noticing, not cryptography.

## 6. Operating notes

* **Logs** carry the request method, path, status, latency, the account UUID
  and the device UUID. Never a token, an auth key, an invite, an email, a
  blob or a header. `crates/havenkeys-server/tests/no_logging.rs` enforces it;
  if you add a log line, run that test.
* **Rate limiting** blocks a login after five failures, escalating 1 → 5 → 30
  minutes, counted per account and per address. The counters live in
  `login_attempts` and a successful login clears them. To unblock someone
  early: `DELETE FROM login_attempts WHERE key = 'acct:<uuid>';`
* **Sessions** last 24 hours and exist only while a device is unlocked. To cut
  a device off immediately, revoke it from the desktop app's Account settings,
  or `UPDATE devices SET revoked_at = now() WHERE id = '<uuid>';` — its
  sessions go with it.
* **Upgrades:** the image runs migrations at boot under a table lock, so two
  instances starting at once is safe. Deploy one replica anyway; nothing here
  has been tested horizontally.
* **Restarting** the server drops nothing but sessions. Devices re-derive
  their auth key at the next unlock and sign in again.

## 7. Running it locally

```sh
docker run -d --name havenkeys-pg -p 5433:5432 \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres \
  postgres:17-alpine

DATABASE_URL='postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable' \
SERVER_SECRET="$(openssl rand -base64 32)" \
cargo run -p havenkeys-server
```

The test suite manages its own disposable databases:

```sh
scripts/test-server.sh
```

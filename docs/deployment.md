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
docker build -t havenkeys-server .
```

## 2. Environment

| Variable | Required | Meaning |
|---|---|---|
| `DATABASE_URL` | yes | `postgres://user:pass@host:5432/db`. TLS is used unless the URL says `sslmode=disable`, and the certificate must be signed by a public root — this client does not accept a self-signed one. Managed databases on a **private** network (Railway's among them) present exactly that, so those URLs need `?sslmode=disable`; the private network is the boundary doing the work there. |
| `SERVER_SECRET` | yes | 32 random bytes, base64. Used only to derive the decoy account id and salt that keep `auth/params` from revealing whether an email has an account. |
| `PORT` | no | Defaults to 8080. Railway sets it. |
| `HAVENKEYS_CORS_ORIGIN` | no | Exactly one browser origin. Leave unset: nothing in the MVP calls this API from a browser, and there is no wildcard. |
| `HAVENKEYS_TRUST_FORWARDED_FOR` | no | `1` only when a proxy you control sets `X-Forwarded-For`. Off by default, because a client could otherwise spread its login attempts over invented addresses and escape the per-IP rate limit. **Known weakness — read the note below before enabling it.** |

> **`X-Forwarded-For` is read left-to-right, which is the wrong end.** With
> this on, the server takes the *first* entry of the header and uses it as the
> rate-limit key without parsing it as an IP address. Proxies append the peer
> address to whatever the client sent, so the first entry is client-controlled.
> An attacker can therefore vary it to get a fresh per-IP budget on every
> request, put a victim's address there to have the victim blocked, or grow the
> `login_attempts` table without bound. The per-account limit still applies,
> and the auth key is 256-bit, so this is a rate-limiting and storage problem
> rather than a route to guessing a password — but the per-IP limit is the only
> thing throttling invite guessing on `POST /v1/accounts/activate`. Until the
> server takes the Nth entry from the right and validates it as an IP, treat
> the per-address limit as advisory, and put the throttling you actually rely
> on in your proxy or edge.

Generate the secret once and store it as a platform variable:

```sh
openssl rand -base64 32
```

Changing `SERVER_SECRET` later is harmless to vault data — it protects no
ciphertext — but it changes the decoy answers, which is visible to anyone
probing `auth/params`.

Migrations run at startup, so a fresh database needs no manual step.

## 3. Railway, step by step

Two files at the repository root do the configuring: the `Dockerfile`, which
Railway uses for any service it finds one for, and `railway.json`, which adds
the health check and the restart policy. The build context is the repository
root, because the build has to read the whole Cargo workspace to resolve it —
which is also why the Dockerfile lives at the root rather than beside the
crate.

### 3.1 Push the branch

Railway deploys what GitHub has:

```sh
git push origin main
```

### 3.2 Create the project and the database

1. railway.com → **New Project** → **Deploy PostgreSQL**. That alone creates
   the project with a Postgres service in it.
2. Note the project's environment (`production` by default).

### 3.3 Add the server service

1. In the same project: **New** → **GitHub Repo** → this repository.
2. Railway finds the root `Dockerfile` and builds with it. The first build
   takes a few minutes: it compiles the crate from scratch.

If the build log says **Railpack** and `No start command detected`, Railway
did not see the Dockerfile. Check, on the service's **Settings**:

* **Source → Root Directory** must be empty or `/`. Anything else and Railway
  looks for the Dockerfile and `railway.json` inside that directory instead.
* **Build → Builder** must be Dockerfile (or automatic). A builder pinned to
  Railpack in the dashboard wins over what the repository contains.
* **Config-as-code** path must be empty or `/railway.json`.
* The deployment must be from a commit that *has* these files — a service
  created before they were pushed keeps building the commit it knew.
  **Deployments → Redeploy** on the latest commit.

### 3.4 Set the variables

On the **server** service → **Variables**:

| Variable | Value |
|---|---|
| `DATABASE_URL` | `${{Postgres.DATABASE_URL}}?sslmode=disable` — a reference, not a copy, so it follows the database. The suffix matters: Railway's Postgres offers TLS with a self-signed certificate, which this client refuses, and its private network is what protects the link instead |
| `SERVER_SECRET` | the output of `openssl rand -base64 32`, generated once |
| `HAVENKEYS_TRUST_FORWARDED_FOR` | `1` — Railway's edge sets `X-Forwarded-For`, and without this every request looks like it comes from the proxy, which would make per-address rate limiting useless (and worse: five failures from anyone would then block every login). Read the warning in §3 first — with it on, the per-address limit is spoofable and should be treated as advisory |
| `PORT` | `8080` |

`PORT` is set explicitly because Railway's docs disagree with themselves about
whether it injects one for a Dockerfile service, and the domain below has to
name the same number. The server binds `0.0.0.0:$PORT` and falls back to 8080,
so an explicit 8080 makes both paths agree.

`${{Postgres.DATABASE_URL}}` is the private-network address. It never leaves
Railway's network and costs no egress.

### 3.5 Give it a domain

Server service → **Settings** → **Networking** → **Generate Domain**, with
**target port 8080** — the same port §3.4 set. Railway asks for it, or guesses
from what the process is listening on; a domain pointing at a port nothing is
bound to is what produces `Application failed to respond`. The deploy log's
`havenkeys-server listening` line prints the port actually bound, which is the
number to check against. Railway terminates TLS on that domain. The server itself speaks plain HTTP and
expects exactly this; it must never be reachable over plain HTTP from
outside, which is why the client refuses any server URL that is not `https://`
(localhost aside).

### 3.6 Check it

```sh
curl -si https://<your-host>/v1/health     # 200 {"status":"ok"}
curl -sS --proto '=https' --tlsv1.2 https://<your-host>/v1/health >/dev/null && echo "TLS ok"
```

The deploy logs should show `migration applied` once, then
`havenkeys-server listening`.

If it cannot reach the database it waits up to a minute, logging
`waiting for the database` with a reason, and then exits with that reason.
The reasons are the whole diagnosis, and none of them prints the URL:

| Log line | What it means |
|---|---|
| `the database host name does not resolve yet` | The private network is not up, or `DATABASE_URL` names a service that does not exist. If it never resolves, check that the reference is `${{Postgres.DATABASE_URL}}` and that the Postgres service really is called `Postgres`. |
| `nothing is listening at that address` | Right host, wrong port — or the database service is not running. |
| `the database rejected these credentials` | The URL is stale: the database was recreated and the variable still holds the old password. Use the reference form, which follows it. |
| `that database does not exist on the server` | The URL's path names a database that was never created. |
| `the database is still starting up` | Normal for a few seconds after the Postgres service deploys; it retries. |
| `the database's TLS certificate is not trusted` | The database presents a self-signed certificate. On Railway's private network that is expected: add `?sslmode=disable` to the variable. Postgres's own log shows this as `could not accept SSL connection: unexpected eof while reading`. |

A failure that waiting cannot fix — wrong credentials, missing database — is
reported immediately rather than after the full minute.

### 3.7 The CLI, for the steps below

```sh
bash <(curl -fsSL railway.com/install.sh)   # install
railway login
railway link                                # pick the project and the server service
```

## 4. The first account

There is no public signup: an account exists because an operator created one.
On Railway that means running the CLI inside the deployed service, where the
binary and the private `DATABASE_URL` both are:

```sh
railway ssh
havenkeys-server admin new-account --email you@example.com \
                                   --server-url https://<your-host>
```

It prints one invite string (`HKINV1-…`), **once**, to that terminal; only its
SHA-256 reaches the database. It is single-use and expires after seven days.
Paste it into the desktop app's first-run screen.

Other commands, in the same shell:

```sh
havenkeys-server admin list-accounts
havenkeys-server admin delete-account --email you@example.com   # irreversible
```

Running the same binary locally works too, as long as it is given a
`DATABASE_URL` that reaches the database (§5 explains the public one).

## 5. Backups, and the restore drill (blocking)

**Railway does not enable scheduled backups by default.** Turn them on before
the vault holds anything you would miss: Postgres service → **Backups** →
choose Daily (kept 6 days), Weekly (27) or Monthly (89).

Those are *volume snapshots*. Restoring one is a dashboard action — it
stages a new volume, unmounts the old one and redeploys — and they cannot be
downloaded. That makes them a good recovery path for "the database broke" and
no protection at all against "the Railway account is gone". Keep your own
dump as well, which is what the drill below produces.

To reach the database from your machine you need the public address: Postgres
service → **Settings** → **Networking** → enable the TCP proxy, which adds
`DATABASE_PUBLIC_URL`. Egress through it is billed, so use it for backups and
turn it off if you would rather not leave it open.

A backup that has never been restored is not a backup. Do this once, and
again after any change to the database plan:

```sh
# 1. Take a dump of the live database, through the public proxy.
export DATABASE_PUBLIC_URL="$(railway variables --service Postgres --kv | grep '^DATABASE_PUBLIC_URL=' | cut -d= -f2-)"
pg_dump --format=custom "$DATABASE_PUBLIC_URL" > havenkeys-$(date +%F).dump

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
  has been tested horizontally. Some releases also need the desktops
  upgraded in step; see §6.1.
* **Restarting** the server drops nothing but sessions. Devices re-derive
  their auth key at the next unlock and sign in again.

### 6.1 Upgrading to the account-fixes release (desktop schema 5)

This release removed `PUT /v1/vault/header` (a password change now goes
through `POST /v1/account/credentials`) and moved the desktop's local store
from schema 4 to schema 5 **with no migration**: after the upgrade a desktop
cannot open its existing `vault.sqlite3`. The vault itself is on the server,
so nothing is lost if the steps are followed in order:

1. **Check first, on the old build.** If any account had its master password
   changed with the old build, its server verifier is stale
   (`security-review.md` S14): every device of that account fails to sign in
   and stays offline. There is no in-place repair: no admin command resets
   a verifier (it is derived from the password on the client), and changing
   the password again on the old build cannot fix it, because that build
   needs a server session to change a password and picks a fresh KDF salt
   each time. The account has to be deleted and recreated with
   `admin delete-account` / `admin new-account`, which loses the items the
   server holds for it.
2. **Upgrade the server.**
3. **Upgrade every desktop.** Change no master password between steps 2
   and 3.
4. **On each desktop**, before starting the new build, move `vault.sqlite3`
   and `device.json` out of the app's data folder to somewhere safe. Start
   the new build, choose **Sign in**, and use the Emergency Kit (email,
   server, Secret Key) and the master password. Delete the moved files once
   the vault has synced.

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

## Desktop installers

Releases are built by `.github/workflows/release.yml`; the installers are
unsigned; see `docs/website.md` for how to cut a release.

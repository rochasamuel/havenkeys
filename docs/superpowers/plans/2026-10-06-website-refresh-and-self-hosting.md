# Website refresh and easy self-hosting — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** havenkeys.net speaks to everyday users first (technical depth moves to a Developers page), and anyone can self-host the server with a prebuilt image, one Compose bundle and one setup script.

**Architecture:** Self-hosting is pure packaging around the existing `Dockerfile`: a GHCR workflow publishes the image; `deploy/compose/` wires server + Postgres + Caddy + a backup loop; docs explain it. The website (`apps/web`, React + Vite, messages in `src/i18n/en.tsx` / `pt-BR.tsx` typed as `Messages`) gets two new pages (`/self-host`, `/developers`), a rewritten Home and Security, and small Download/legal edits. No server, client or protocol code changes.

**Tech Stack:** Docker Compose v2, Caddy 2, Postgres 16, POSIX sh, GitHub Actions (Buildx, GHCR), React 19, react-router 7, Vite, vitest (node environment, `react-dom/server` for render tests), Playwright (tools/ui-check).

**Spec:** `docs/superpowers/specs/2026-10-06-website-refresh-and-self-hosting-design.md`

## Global Constraints

- No change to any Rust crate, the desktop app, the extension or Android.
- Invite address exactly `invite@havenkeys.net`; subject en `HavenKeys invite request`, pt-BR `Pedido de convite HavenKeys`.
- Image name exactly `ghcr.io/rochasamuel/havenkeys-server`; release tags `server-v<semver>` → image tags `<semver>` and `latest`; platforms `linux/amd64`, `linux/arm64`.
- Compose: Postgres `postgres:16-alpine`, Caddy `caddy:2-alpine`; only Caddy publishes ports; database never published; `.env` written with mode 600; secrets never echoed; backups kept `BACKUP_KEEP_DAYS` (default 14) days in `./backups`.
- `havenkeys-server admin new-account` needs `--email` and `--server-url https://<domain>`.
- Site CSP stays as in `apps/web/vercel.json` (no inline scripts/styles, images from self only). No new runtime dependencies in `apps/web`.
- Every message key exists in both `en.tsx` and `pt-BR.tsx` (enforced by the `Messages` type). Product names (Secret Key, Emergency Kit, havenkeys-server) stay English in both.
- Plain words on Home, Security, Self-host and Download: no "Argon2id", "HKDF", "AEAD", "ciphertext", "Rust core" there. Those live on Developers.
- Honest claims only: no "unhackable", "military-grade", "100% secure". The audit disclaimer (`t.common.disclaimer`) appears on Security, Developers and in the footer.
- Terms text changes are a draft for the owner to approve before merge.
- No Claude co-author trailer on commits. Commit after each task.

## Review Focus

- `setup.sh` run a second time with an existing `.env` → must not overwrite secrets or ask again; just starts and checks health → test in Task 1.
- A domain typed with a scheme or trailing slash (`https://vault.example.com/`) → setup normalizes to the bare host or refuses with a clear message, never writes a broken Caddyfile host → test in Task 1.
- Old links `https://havenkeys.net/#journey` and `/#browser` → land on a section of the new Home (ids kept), not the top of an unrelated page → test in Task 5.
- `RAILWAY_TEMPLATE_URL` empty → the Self-host page shows the "coming soon" line and no dead button → test in Task 6.
- Phone-width rendering of the new pages (390px) → no horizontal overflow, no clipped text → Task 7 ui-check.

---

### Task 1: Compose bundle

**Files:**
- Create: `deploy/compose/compose.yaml`, `deploy/compose/Caddyfile`, `deploy/compose/.env.example`, `deploy/compose/setup.sh`, `deploy/compose/restore.sh`, `deploy/compose/backup.sh`, `deploy/compose/README.md`, `deploy/compose/test/smoke.sh`

**Interfaces:**
- Produces: the bundle file names (used verbatim by Task 3's guide and Task 6's download command): `compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh`. Service names `server`, `db`, `caddy`, `backup`. Env vars `HAVENKEYS_DOMAIN ACME_EMAIL SERVER_SECRET POSTGRES_PASSWORD HAVENKEYS_IMAGE HAVENKEYS_VERSION BACKUP_KEEP_DAYS TZ HTTP_PORT HTTPS_PORT`.

- [ ] **Step 1: `compose.yaml`**

```yaml
# HavenKeys server, its database, HTTPS and nightly backups.
# Setup: ./setup.sh   Guide: docs/self-hosting.md
name: havenkeys

services:
  server:
    image: "${HAVENKEYS_IMAGE:-ghcr.io/rochasamuel/havenkeys-server}:${HAVENKEYS_VERSION:-latest}"
    restart: unless-stopped
    environment:
      DATABASE_URL: "postgres://havenkeys:${POSTGRES_PASSWORD:?run ./setup.sh first}@db:5432/havenkeys?sslmode=disable"
      SERVER_SECRET: "${SERVER_SECRET:?run ./setup.sh first}"
      # Caddy is the only peer and appends the client address. Read the
      # X-Forwarded-For caveat in docs/deployment.md §2.
      HAVENKEYS_TRUST_FORWARDED_FOR: "1"
      PORT: "8080"
    depends_on:
      db:
        condition: service_healthy

  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: havenkeys
      POSTGRES_DB: havenkeys
      POSTGRES_PASSWORD: "${POSTGRES_PASSWORD:?run ./setup.sh first}"
    volumes:
      - db-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U havenkeys -d havenkeys"]
      interval: 5s
      timeout: 5s
      retries: 20

  caddy:
    image: caddy:2-alpine
    restart: unless-stopped
    ports:
      - "${HTTP_PORT:-80}:80"
      - "${HTTPS_PORT:-443}:443"
    environment:
      HAVENKEYS_DOMAIN: "${HAVENKEYS_DOMAIN:?run ./setup.sh first}"
      ACME_EMAIL: "${ACME_EMAIL:-}"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config
    depends_on:
      - server

  backup:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      PGHOST: db
      PGUSER: havenkeys
      PGDATABASE: havenkeys
      PGPASSWORD: "${POSTGRES_PASSWORD:?run ./setup.sh first}"
      BACKUP_KEEP_DAYS: "${BACKUP_KEEP_DAYS:-14}"
      TZ: "${TZ:-UTC}"
    volumes:
      - ./backups:/backups
      - ./backup.sh:/usr/local/bin/havenkeys-backup:ro
    entrypoint: ["/bin/sh", "/usr/local/bin/havenkeys-backup", "loop"]
    depends_on:
      db:
        condition: service_healthy

volumes:
  db-data:
  caddy-data:
  caddy-config:
```

The server image has no shell tools, so it has no container healthcheck; `setup.sh` checks `/v1/health` through Caddy instead.

- [ ] **Step 2: `Caddyfile`**

```caddy
# HTTPS for your HavenKeys server. Caddy gets and renews the certificate.
{
	email {$ACME_EMAIL}
}

{$HAVENKEYS_DOMAIN} {
	header Strict-Transport-Security "max-age=31536000"
	reverse_proxy server:8080
}
```

- [ ] **Step 3: `backup.sh`**

```sh
#!/bin/sh
# Database backups for the HavenKeys Compose bundle.
#   havenkeys-backup now    one dump now
#   havenkeys-backup loop   one dump every day at 03:00 (container TZ)
# Dumps: /backups/havenkeys-YYYY-MM-DD.dump (pg_dump custom format).
set -eu

dump() {
	out="/backups/havenkeys-$(date +%F).dump"
	tmp="$out.partial"
	pg_dump --format=custom --file="$tmp"
	mv "$tmp" "$out"
	find /backups -name 'havenkeys-*.dump' -type f -mtime +"${BACKUP_KEEP_DAYS:-14}" -delete
	echo "backup: wrote $(basename "$out")"
}

case "${1:-now}" in
now) dump ;;
loop)
	echo "backup: daily at 03:00 ($TZ), keeping ${BACKUP_KEEP_DAYS:-14} days"
	while :; do
		if [ "$(date +%H:%M)" = "03:00" ]; then
			dump || echo "backup: FAILED" >&2
			sleep 61
		fi
		sleep 30
	done
	;;
*) echo "usage: havenkeys-backup now|loop" >&2; exit 2 ;;
esac
```

- [ ] **Step 4: `.env.example`**

```sh
# Copy to .env (./setup.sh does this for you). Keep .env private.

# The address your apps will use, without https:// (e.g. vault.example.com).
HAVENKEYS_DOMAIN=
# Email for Let's Encrypt certificate notices.
ACME_EMAIL=
# 32 random bytes, base64: openssl rand -base64 32
SERVER_SECRET=
# Database password: any long random string.
POSTGRES_PASSWORD=
# Pin a release (e.g. 0.17.0) to upgrade on your schedule. Default: latest.
HAVENKEYS_VERSION=latest
# Days of nightly backups to keep in ./backups.
BACKUP_KEEP_DAYS=14
# Time zone for the 03:00 backup, e.g. America/Sao_Paulo.
TZ=UTC
```

- [ ] **Step 5: `setup.sh`**

```sh
#!/bin/sh
# One-time setup (safe to re-run): writes .env, starts HavenKeys, checks it.
set -eu
cd "$(dirname "$0")"

say() { printf '%s\n' "$*"; }
die() { printf 'setup: %s\n' "$*" >&2; exit 1; }

command -v docker >/dev/null 2>&1 || die "Docker is not installed. See https://docs.docker.com/engine/install/"
docker compose version >/dev/null 2>&1 || die "Docker Compose v2 is missing (the 'docker compose' command)."
for f in compose.yaml Caddyfile backup.sh; do [ -f "$f" ] || die "$f is missing; download the whole bundle."; done

random_b64() {
	if command -v openssl >/dev/null 2>&1; then openssl rand -base64 "$1"
	else head -c "$1" /dev/urandom | base64 | tr -d '\n'; fi
}

# A bare host name: no scheme, no path, no port, no spaces.
clean_domain() {
	printf '%s' "$1" | sed -e 's#^[a-zA-Z]*://##' -e 's#/.*$##' -e 's#:.*$##' | tr 'A-Z' 'a-z'
}

if [ ! -f .env ]; then
	say "HavenKeys server setup"
	printf 'Domain for this server (e.g. vault.example.com): '
	read -r raw
	domain=$(clean_domain "$raw")
	case "$domain" in
	"" | *[!a-z0-9.-]* | .* | *.) die "\"$raw\" is not a domain name." ;;
	esac
	printf 'Email for certificate notices: '
	read -r email
	case "$email" in *@*.*) ;; *) die "\"$email\" is not an email address." ;; esac
	secret=$(random_b64 32)
	dbpass=$(random_b64 24 | tr -d '/+=')
	umask 077
	{
		say "HAVENKEYS_DOMAIN=$domain"
		say "ACME_EMAIL=$email"
		say "SERVER_SECRET=$secret"
		say "POSTGRES_PASSWORD=$dbpass"
		say "HAVENKEYS_VERSION=latest"
		say "BACKUP_KEEP_DAYS=14"
		say "TZ=UTC"
	} >.env
	chmod 600 .env
	say "Wrote .env (keep it private; it holds the server's secrets)."
else
	say "Using the existing .env."
fi
mkdir -p backups

domain=$(sed -n 's/^HAVENKEYS_DOMAIN=//p' .env)
[ -n "$domain" ] || die ".env has no HAVENKEYS_DOMAIN."
if command -v getent >/dev/null 2>&1 && ! getent hosts "$domain" >/dev/null 2>&1; then
	say "Warning: $domain does not resolve yet. Create its DNS record, or HTTPS will fail."
fi

docker compose up -d --pull missing

say "Waiting for https://$domain/v1/health ..."
i=0
while [ $i -lt 24 ]; do
	if curl -fsS "https://$domain/v1/health" >/dev/null 2>&1; then
		say "HavenKeys is running at https://$domain"
		say ""
		say "Create your account:"
		say "  docker compose exec server havenkeys-server admin new-account \\"
		say "    --email you@example.com --server-url https://$domain"
		exit 0
	fi
	i=$((i + 1))
	sleep 5
done
say "Not answering yet. Check the logs:"
say "  docker compose logs caddy server"
exit 1
```

- [ ] **Step 6: `restore.sh`**

```sh
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
```

- [ ] **Step 7: `README.md`**

```markdown
# HavenKeys server — Docker Compose bundle

Server, database, automatic HTTPS (Caddy) and nightly backups.
Run `./setup.sh`. The full guide is [docs/self-hosting.md](../../docs/self-hosting.md).
```

Run `chmod +x deploy/compose/*.sh` and commit the mode (`git update-index --chmod=+x`).

- [ ] **Step 8: Smoke test script `deploy/compose/test/smoke.sh`**

```sh
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
cat >.env <<EOF
HAVENKEYS_DOMAIN=localhost
ACME_EMAIL=
SERVER_SECRET=$(head -c 32 /dev/urandom | base64 | tr -d '\n')
POSTGRES_PASSWORD=smoke$(head -c 12 /dev/urandom | base64 | tr -d '/+=\n')
HAVENKEYS_IMAGE=havenkeys-server
HAVENKEYS_VERSION=smoke
HTTP_PORT=18080
HTTPS_PORT=18443
EOF
docker compose config -q
docker compose up -d
i=0
until curl -fsSk https://localhost:18443/v1/health; do
	i=$((i + 1)); [ $i -lt 30 ] || { docker compose logs; exit 1; }; sleep 2
done
echo
docker compose exec -T backup /bin/sh /usr/local/bin/havenkeys-backup now
ls backups | grep -q '^havenkeys-.*\.dump$'
docker compose exec -T server havenkeys-server admin list-accounts
echo "smoke: OK"
```

- [ ] **Step 9: Domain-normalization check (Review Focus)**

Run, from `deploy/compose`, this throwaway check of `clean_domain` (copy the function into a shell):

```sh
sh -c '. /dev/stdin; for d in "https://Vault.Example.com/" "vault.example.com:443" "vault.example.com"; do clean_domain "$d"; echo; done' <<'EOF'
clean_domain() { printf '%s' "$1" | sed -e 's#^[a-zA-Z]*://##' -e 's#/.*$##' -e 's#:.*$##' | tr 'A-Z' 'a-z'; }
EOF
```

Expected: three lines `vault.example.com`.

Then verify re-run safety: in a temp dir with the bundle and a pre-made `.env`, run `HAVENKEYS_DOMAIN` check by `sh -n setup.sh` (syntax) and confirm by reading that the `.env` branch is skipped when the file exists. (The smoke test covers starting.)

- [ ] **Step 10: Run the smoke test**

Run: `sh deploy/compose/test/smoke.sh` (repo root; the first image build compiles the server and takes several minutes; use a 20-minute timeout).
Expected: `{"status":"ok"}`, `backup: wrote havenkeys-….dump`, the (empty) account list, `smoke: OK`. If ports 18080/18443 are busy, change them in the script. If Docker cannot run here, report it with the exact error.

Also run `sh -n` on all four scripts (syntax check). `shellcheck` is not installed; skip it.

- [ ] **Step 11: Commit**

```bash
git add deploy/compose
git commit -m "feat(deploy): Docker Compose bundle with HTTPS, backups and a setup script"
```

---

### Task 2: Prebuilt server image on GHCR

**Files:**
- Create: `.github/workflows/server-image.yml`

**Interfaces:**
- Produces: `ghcr.io/rochasamuel/havenkeys-server:<version>` and `:latest`, multi-arch, on `server-v*` tags.

- [ ] **Step 1: Write the workflow**

Native runners per architecture (compiling Rust under QEMU would take far too long), then one job joins the two into a multi-arch tag. Actions are pinned by commit SHA like the other workflows (SHAs below were looked up on 2026-10-06 from each action's latest release).

```yaml
name: Server image

# Publishes ghcr.io/rochasamuel/havenkeys-server for linux/amd64 and
# linux/arm64. Push a tag server-v<version> (e.g. server-v0.17.0).
on:
  push:
    tags: ["server-v*"]
  workflow_dispatch:
    inputs:
      version:
        description: "Image version tag (e.g. 0.17.0)"
        required: true

permissions:
  contents: read
  packages: write

env:
  IMAGE: ghcr.io/rochasamuel/havenkeys-server

jobs:
  build:
    strategy:
      matrix:
        include:
          - arch: amd64
            runner: ubuntu-24.04
          - arch: arm64
            runner: ubuntu-24.04-arm
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
      - uses: docker/setup-buildx-action@f87e5991a6d7451dcb8d9637bfbc97413f497069 # v4.4.1
      - uses: docker/login-action@dbcb813823bdd20940b903addbd779551569679f # v4.6.0
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Version
        id: v
        shell: bash
        run: |
          v="${{ inputs.version }}"
          [ -n "$v" ] || v="${GITHUB_REF_NAME#server-v}"
          echo "version=$v" >> "$GITHUB_OUTPUT"
      - uses: docker/build-push-action@c3c9e263c25d99ce0380d002d59b67737d91b0dc # v7.4.0
        with:
          context: .
          platforms: linux/${{ matrix.arch }}
          push: true
          tags: ${{ env.IMAGE }}:${{ steps.v.outputs.version }}-${{ matrix.arch }}
          cache-from: type=gha,scope=${{ matrix.arch }}
          cache-to: type=gha,mode=max,scope=${{ matrix.arch }}
          provenance: false
    outputs:
      version: ${{ steps.v.outputs.version }}

  manifest:
    needs: build
    runs-on: ubuntu-24.04
    steps:
      - uses: docker/setup-buildx-action@f87e5991a6d7451dcb8d9637bfbc97413f497069 # v4.4.1
      - uses: docker/login-action@dbcb813823bdd20940b903addbd779551569679f # v4.6.0
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Multi-arch tags
        env:
          V: ${{ needs.build.outputs.version }}
        run: |
          for tag in "$V" latest; do
            docker buildx imagetools create -t "$IMAGE:$tag" "$IMAGE:$V-amd64" "$IMAGE:$V-arm64"
          done
```


- [ ] **Step 2: Validate**

`actionlint` is not installed. Validate the YAML parses: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/server-image.yml'))"` (install PyYAML with `pip install --user pyyaml` if missing, or use `node -e` with the `yaml` package if present in node_modules). Read the file once more against this step's text.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/server-image.yml
git commit -m "ci: publish the server image to GHCR for amd64 and arm64 on server-v tags"
```

---

### Task 3: Self-hosting docs

**Files:**
- Create: `docs/self-hosting.md`, `deploy/railway/README.md`
- Modify: `docs/deployment.md` (top), `README.md` (one link, if it has a docs list: `grep -n "deployment.md" README.md`)

**Interfaces:**
- Consumes: Task 1 file names, service names, env vars; Task 2 image name and tag scheme.

- [ ] **Step 1: `docs/self-hosting.md`**

Write it for a non-expert, in this order, with these exact commands:

1. Title `# Run your own HavenKeys server`, the audit disclaimer quote line used in other docs, and one paragraph: what you get (your encrypted vault on a server you control; the server can't read it; your apps sync through it).
2. `## What you need` — Option A: a Linux server with Docker (1 GB RAM is enough; about US$5/month at most providers, or an always-on computer at home reachable from the internet) and a domain or subdomain you control. Option B: a Railway account.
3. `## Option A: your own server with Docker`
   1. *Point your domain at the server*: an `A` record (and `AAAA` if you have IPv6) for e.g. `vault.example.com`; open ports 80 and 443 (`ufw allow 80,443/tcp` as an example).
   2. *Download the bundle*:
      ```sh
      mkdir havenkeys && cd havenkeys
      for f in compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh; do
        curl -fsSLO "https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/$f"
      done
      chmod +x setup.sh restore.sh
      ```
   3. *Run the setup*: `./setup.sh` — what it asks, what it writes (`.env`, mode 600 — back it up somewhere safe: without `SERVER_SECRET` and the database password you can't restore), what success looks like.
   4. *Create your account*:
      ```sh
      docker compose exec server havenkeys-server admin new-account \
        --email you@example.com --server-url https://vault.example.com
      ```
      The `HKINV1-…` invite prints once; single use, expires in 7 days; paste it into the HavenKeys app's first screen (desktop) and print the Emergency Kit. Other admin commands: `list-accounts`, `delete-account` (irreversible).
4. `## Option B: Railway` — if the template is published, the button link (say: "See the Self-host page on havenkeys.net for the button"); otherwise point to `docs/deployment.md` §3 for the manual steps. Mention cost (~US$5/month) and that Railway backups must be turned on (deployment.md §5).
5. `## Connect your apps` — the server address is `https://<your domain>`; desktop first-run screen → invite; Android → scan the Emergency Kit or sign in; extension needs the desktop app.
6. `## Backups (do this before storing real passwords)` — nightly dump at 03:00 (TZ in `.env`), 14 days kept in `./backups`; copy them off the machine (example: `rsync -a havenkeys/backups/ user@other-host:havenkeys-backups/`); take one now: `docker compose exec backup havenkeys-backup now`; restore drill: `./restore.sh backups/havenkeys-YYYY-MM-DD.dump` on a test copy, then open an app and check items. State plainly: the server is the authoritative copy; no backup, no recovery.
7. `## Upgrading` — `docker compose pull && docker compose up -d`; pin `HAVENKEYS_VERSION=0.17.0` in `.env` to choose when; take a backup first; migrations run on start.
8. `## Troubleshooting` — certificate not issued (DNS not pointing here yet, ports 80/443 closed, another web server using them: `docker compose logs caddy`); `/v1/health` fails (`docker compose logs server`; "waiting for the database" means `db` is not healthy: `docker compose logs db`); forgot the invite (create a new account after `delete-account`, only if it was never used).
9. `## Security notes` — HTTPS only (apps refuse plain http); `HAVENKEYS_TRUST_FORWARDED_FOR=1` is set because Caddy is the only peer; read the `X-Forwarded-For` caveat in `deployment.md` §2; keep the host updated; never expose Postgres; the software has not been independently audited.
10. `## Reference` — link `deployment.md` for all environment variables and platform notes.

- [ ] **Step 2: `deploy/railway/README.md`** — owner steps to publish the template:

```markdown
# Publishing the Railway template (owner)

1. In Railway, open the project that runs havenkeys-server (Postgres + server).
2. Project → Settings → **Generate Template from Project**.
3. In the template editor:
   - Postgres service: keep as is.
   - Server service: source = this GitHub repo, root directory empty (uses `Dockerfile` and `railway.json`).
   - Variables:
     - `DATABASE_URL` = `${{Postgres.DATABASE_URL}}?sslmode=disable`
     - `SERVER_SECRET` = empty, with the description "Run `openssl rand -base64 32` and paste the result" (the server requires base64 of exactly 32 bytes, which Railway's generators don't produce; Railway prompts the deployer for empty variables)
     - `HAVENKEYS_TRUST_FORWARDED_FOR` = `1`
     - `PORT` = `8080`
   - Networking: public domain on port 8080.
4. Publish. Copy the template URL (https://railway.com/deploy/...).
5. Put it in `apps/web/src/lib/links.ts` as `RAILWAY_TEMPLATE_URL` and deploy the site.
```

- [ ] **Step 3: Pointers**

Top of `docs/deployment.md`, after the disclaimer: `> Setting up your own server for the first time? Start with [self-hosting.md](self-hosting.md); this document is the reference.` Add `self-hosting.md` to `README.md`'s docs list if it has one.

- [ ] **Step 4: Commit**

```bash
git add docs/self-hosting.md docs/deployment.md deploy/railway/README.md README.md
git commit -m "docs: run your own server with Docker Compose or Railway"
```

---

### Task 4: Developers page (move the technical content)

**Files:**
- Create: `apps/web/src/pages/Developers.tsx` (from `git mv apps/web/src/pages/Security.tsx apps/web/src/pages/Developers.tsx`), `apps/web/src/App.test.tsx`
- Modify: `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx`, `apps/web/src/App.tsx`, `apps/web/src/pages/Home.tsx` (temporarily untouched except imports if needed)

**Interfaces:**
- Produces: messages section `developers` (both locales) = every key of the current `security` section **plus** these keys moved out of `home`: `journeyTitle journeyLede browserTitle browserLede extensionListTitle extensionFeatures desktopTitle desktopLede features paperTitle paperP1 paperP2 paperWarn ledgerTitle ledgerLede defendsTitle defends doesntTitle doesnt`, plus new keys `buildTitle buildBody`. Route `/developers` (+ `/pt-br/developers`). `export function Site()` in `App.tsx` (tests render it).
- The old `security` section is replaced in Task 5; in this task, copy (don't move) the `security` keys into `developers` so Security.tsx still compiles. Leave `home` keys in place too (Task 5 deletes them). Duplication is temporary and removed in Task 5.

- [ ] **Step 1: Failing route test**

Create `apps/web/src/App.test.tsx`:

```tsx
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { Site } from "./App";
import { en } from "./i18n/en";
import { ptBR } from "./i18n/pt-BR";

function html(path: string): string {
  return renderToString(
    <MemoryRouter initialEntries={[path]}>
      <Site />
    </MemoryRouter>,
  );
}

const text = (s: string) => s.replace(/&#x27;|’/g, "'");

describe("routes", () => {
  it("renders the Developers page in both languages", () => {
    expect(text(html("/developers"))).not.toContain(text(en.notFound.title));
    expect(text(html("/pt-br/developers"))).not.toContain(text(ptBR.notFound.title));
  });
  it("still renders every existing page", () => {
    for (const p of ["/", "/download", "/security", "/privacy", "/terms", "/delete-account"]) {
      expect(text(html(p)), p).not.toContain(text(en.notFound.title));
      expect(text(html(`/pt-br${p === "/" ? "" : p}`)), p).not.toContain(text(ptBR.notFound.title));
    }
  });
});
```

If `en.notFound.title` is not a plain string, compare against the rendered `<h1>` of NotFound instead (read `pages/NotFound.tsx`). If rendering with `renderToString` fails on a browser-only API at module level (e.g. `window` in a component body, `Analytics`), fix by rendering `Site` (which excludes `Analytics`) and guarding the access; report what you changed.

Run: `pnpm --filter @havenkeys/web exec vitest run src/App.test.tsx` → FAIL (`Site` not exported / no `/developers` route).

- [ ] **Step 2: Messages**

In `en.tsx` add a `developers` section (after `kit`): copy the whole current `security` object's content, then the listed `home` keys (copy text verbatim), and change/add:

```tsx
    heroTitle: "How HavenKeys works, end to end.",
    // heroLede: keep the current security.heroLede text
    buildTitle: "Build it yourself",
    buildBody: (
      <>
        HavenKeys is open source under MIT or Apache-2.0. The{" "}
        <Ext href={`${DOCS}development.md`}>development guide</Ext> covers building the desktop app,
        the extension, Android and the server; <Ext href={`${DOCS}self-hosting.md`}>self-hosting.md</Ext>{" "}
        covers running your own server.
      </>
    ),
```

Update the copied `features` entry `Import from 1Password` to:

```tsx
      { term: "Import and export", text: "Bring passwords from 1Password, Bitwarden, LastPass, KeePassXC, Chrome or Firefox. Export a Bitwarden file, a CSV, or an encrypted HavenKeys backup." },
```

Add to the copied `reading` list (keep its item shape — read it first) two entries for `self-hosting.md` ("Self-hosting: run your own server with Docker or Railway") and `deployment.md` ("Deployment reference: environment, Railway, backups").

`pt-BR.tsx`: the same, from the current `ptBR.security` and `ptBR.home` values, with:

```tsx
    heroTitle: "Como o HavenKeys funciona, de ponta a ponta.",
    buildTitle: "Compile você mesmo",
    buildBody: (
      <>
        O HavenKeys é código aberto, sob MIT ou Apache-2.0. O{" "}
        <Ext href={`${DOCS}development.md`}>guia de desenvolvimento</Ext> explica como compilar o app
        de desktop, a extensão, o Android e o servidor; o{" "}
        <Ext href={`${DOCS}self-hosting.md`}>self-hosting.md</Ext> explica como rodar seu próprio servidor.
      </>
    ),
```

and the features entry `{ term: "Importar e exportar", text: "Traga senhas do 1Password, Bitwarden, LastPass, KeePassXC, Chrome ou Firefox. Exporte um arquivo do Bitwarden, um CSV ou um backup criptografado do HavenKeys." }`, and reading entries "Auto-hospedagem: rode seu próprio servidor com Docker ou Railway" / "Referência de implantação: variáveis, Railway, backups". (Check the existing pt term names to match their style; `Ext`/`DOCS` helpers exist in pt-BR.tsx too — confirm with `grep -n "function Ext\|const DOCS" apps/web/src/i18n/pt-BR.tsx`.)

- [ ] **Step 3: Page**

`git mv apps/web/src/pages/Security.tsx apps/web/src/pages/Developers.tsx`, then recreate `Security.tsx` as an exact copy of the moved file's original content (so `/security` keeps working until Task 5): `git show HEAD:apps/web/src/pages/Security.tsx > apps/web/src/pages/Security.tsx`.

In `Developers.tsx`: rename the component to `Developers`, use `const s = t.developers`, wrap in `<div className="security developers">`, and insert the moved Home sections (copy their JSX from `Home.tsx`, switching `h.` to `s.`) in this order after the hero: the Journey section (`<section className="section intro" id="journey">…<Journey/>`), then the key chain (existing), then the paper chapter (`<section className="paper">…<EmergencyKit/>`), then the browser section (`id="browser"`, `BrowserShowcase`, store buttons, extension list), then the desktop features section, then the existing zones / permissions / attacks sections, then the ledger section (without its "Read the security overview" link), then scope, reading, and a final section:

```tsx
      <section className="section">
        <div className="section__head">
          <h2>{s.buildTitle}</h2>
          <p>{s.buildBody}</p>
        </div>
      </section>
```

Imports: `Journey`, `BrowserShowcase`, `EmergencyKit`, `Icon`, `useI18n`, the store URLs (copy the constants from Home.tsx).

- [ ] **Step 4: Route**

In `App.tsx`: `export function Site()`, import `Developers`, add `{ path: "developers", element: <Developers /> }` to `PAGES`.

- [ ] **Step 5: Run and commit**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web build` → pass.

```bash
git add apps/web
git commit -m "feat(web): a Developers page with the technical material"
```

---

### Task 5: New Home and plain-language Security

**Files:**
- Create: `apps/web/src/lib/links.ts`, `apps/web/src/lib/links.test.ts`, `apps/web/src/i18n/messages.test.ts`
- Modify: `apps/web/src/pages/Home.tsx`, `apps/web/src/pages/Security.tsx`, `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx`, `apps/web/src/components/Nav.tsx`, `apps/web/src/components/Footer.tsx`, `apps/web/src/styles/global.css`, `apps/web/src/App.test.tsx`

**Interfaces:**
- Consumes: `developers` messages and `/developers` route (Task 4).
- Produces: `links.ts` exports `INVITE_EMAIL`, `inviteHref(subject: string): string`, `RAILWAY_TEMPLATE_URL: string` (empty), `GITHUB`, `CHROME_STORE`, `FIREFOX_STORE`; messages `common.requestInvite`, `common.inviteSubject`; `nav.selfHost`, `nav.developers`; `footer.selfHost`, `footer.developers`; new `home` and `security` shapes below. Task 6 uses `inviteHref`, `RAILWAY_TEMPLATE_URL`, `nav/footer.selfHost` and adds the `/self-host` route.

- [ ] **Step 1: Failing tests**

`apps/web/src/lib/links.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { INVITE_EMAIL, inviteHref, RAILWAY_TEMPLATE_URL } from "./links";

describe("links", () => {
  it("builds the invite mailto with an encoded subject", () => {
    expect(INVITE_EMAIL).toBe("invite@havenkeys.net");
    expect(inviteHref("Pedido de convite HavenKeys")).toBe("mailto:invite@havenkeys.net?subject=Pedido%20de%20convite%20HavenKeys");
  });
  it("has no Railway template until the owner publishes one", () => {
    expect(RAILWAY_TEMPLATE_URL).toBe("");
  });
});
```

`apps/web/src/i18n/messages.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ptBR } from "./pt-BR";

const TECH = /argon2|hkdf|aead|ciphertext|rust core|xchacha|aes-/i;

describe("messages", () => {
  it("has six main features on Home in both languages", () => {
    expect(en.home.cards).toHaveLength(6);
    expect(ptBR.home.cards).toHaveLength(6);
  });
  it("keeps jargon off the everyday pages", () => {
    for (const m of [en, ptBR]) {
      const plain = JSON.stringify([m.home, m.security]);
      expect(plain).not.toMatch(TECH);
    }
  });
  it("uses the invite subjects from the spec", () => {
    expect(en.common.inviteSubject).toBe("HavenKeys invite request");
    expect(ptBR.common.inviteSubject).toBe("Pedido de convite HavenKeys");
  });
});
```

(`JSON.stringify` of JSX values yields objects with their string children; good enough to catch jargon in strings. If a message is a function, it is skipped by JSON — acceptable.)

Add to `App.test.tsx`:

```tsx
  it("keeps the old Home anchors", () => {
    const home = html("/");
    expect(home).toContain('id="journey"');
    expect(home).toContain('id="browser"');
  });
  it("offers the invite on Home", () => {
    expect(html("/")).toContain("mailto:invite@havenkeys.net?subject=HavenKeys%20invite%20request");
  });
```

Run them → FAIL.

- [ ] **Step 2: `links.ts`**

```ts
// Addresses the site links to, in one place.
export const GITHUB = "https://github.com/rochasamuel/havenkeys";
export const CHROME_STORE = "https://chromewebstore.google.com/detail/havenkeys/fmmfkakdkkcfpdnfmbngnlelbfaogafo";
export const FIREFOX_STORE = "https://addons.mozilla.org/firefox/addon/havenkeys/";
export const INVITE_EMAIL = "invite@havenkeys.net";
/** Set once the owner publishes the Railway template (deploy/railway/README.md). */
export const RAILWAY_TEMPLATE_URL = "";

export function inviteHref(subject: string): string {
  return `mailto:${INVITE_EMAIL}?subject=${encodeURIComponent(subject)}`;
}
```

Use these constants in Home.tsx and Developers.tsx instead of local copies.

- [ ] **Step 3: Messages — `common`, `meta`, `nav`, `footer`**

en:

```tsx
  meta: {
    title: "HavenKeys — passwords only you can open",
    description:
      "HavenKeys is a free, open-source password manager for your computer, phone and browser. Your vault is locked on your devices; nobody else can open it.",
  },
  // common: add
    requestInvite: "Request an invite",
    inviteSubject: "HavenKeys invite request",
  // nav: remove howItWorks, browser; add
    selfHost: "Self-host",
    developers: "Developers",
  // footer: add
    selfHost: "Self-host",
    developers: "Developers",
    tagline: "Passwords only you can open.",
```

pt-BR:

```tsx
  meta: {
    title: "HavenKeys — senhas que só você abre",
    description:
      "O HavenKeys é um gerenciador de senhas gratuito e de código aberto para seu computador, celular e navegador. Seu cofre fica trancado nos seus dispositivos; ninguém mais consegue abrir.",
  },
    requestInvite: "Pedir um convite",
    inviteSubject: "Pedido de convite HavenKeys",
    selfHost: "Auto-hospedar",
    developers: "Desenvolvedores",
    // footer.tagline:
    tagline: "Senhas que só você abre.",
```

- [ ] **Step 4: Messages — new `home`**

Replace the whole `home` section (the technical keys now live in `developers`). en:

```tsx
  home: {
    heroTitle: (
      <>
        Your passwords, locked — and only <em>yours</em>.
      </>
    ),
    heroLede:
      "A password manager for your computer, your phone and your browser. Your vault is locked on your own devices, and nobody else — not even the server that keeps your copy — can open it.",
    heroMeta: "Free and open source · Windows, macOS, Linux and Android · Chrome and Firefox",
    desktopAlt: /* keep the current text */,
    popupAlt: /* keep */,
    menuAlt: /* keep */,

    cardsTitle: "Everything you need. Nothing you don’t.",
    cardsLede: "The essentials of a modern password manager, made with care.",
    cards: [
      { icon: "key", title: "Fills logins when you click", text: "On a sign-in page, HavenKeys offers what you saved for that site and waits for you to choose." },
      { icon: "lock", title: "Passkeys", text: "Create passkeys and sign in with them, on your computer and your phone." },
      { icon: "check", title: "One-time codes built in", text: "Two-step codes live next to the password. No separate authenticator app." },
      { icon: "laptop", title: "Works offline", text: "Every device keeps its own locked copy, so your passwords are there even without internet." },
      { icon: "monitor", title: "On all your devices", text: "Desktop app, Android app and a browser extension for Chrome and Firefox, kept in sync." },
      { icon: "download", title: "Bring your passwords", text: "Import from 1Password, Bitwarden, LastPass, KeePassXC, Chrome or Firefox. Export any time." },
    ],

    trustTitle: "Nobody else can open it.",
    trustBody:
      "Your vault is locked on your device with two things only you have: your master password and a Secret Key HavenKeys creates for you. What reaches the server is already locked, so the people who run it — us included — can’t read a single password.",
    trustLink: "How it works, in detail",

    stepsTitle: "Start in three steps.",
    steps: [
      { title: "Install HavenKeys", text: "Get the app for Windows, macOS, Linux or Android, and the extension for Chrome or Firefox." },
      { title: "Get your account", text: "Request an invite to our server, or run your own." },
      { title: "Print your Emergency Kit", text: "It holds your Secret Key. Keep it somewhere safe: it’s how you get back in on a new device." },
    ],

    selfHostTitle: "Prefer your own server?",
    selfHostBody:
      "Run HavenKeys on a small server of your own or on Railway. Setup takes a few commands, and nightly backups are included.",
    selfHostCta: "Run your own server",

    closerTitle: /* keep the current JSX */,
    closerLede: "Install the app, request an invite, and add the extension.",
    readSource: "Read the source",
  },
```

pt-BR (same keys):

```tsx
    heroTitle: (<>Suas senhas, trancadas — e só <em>suas</em>.</>),
    heroLede: "Um gerenciador de senhas para o seu computador, celular e navegador. Seu cofre fica trancado nos seus próprios dispositivos, e ninguém mais — nem o servidor que guarda a sua cópia — consegue abri-lo.",
    heroMeta: /* keep current pt text */,
    cardsTitle: "Tudo o que você precisa. Nada que não precisa.",
    cardsLede: "O essencial de um gerenciador de senhas moderno, feito com cuidado.",
    cards: [
      { icon: "key", title: "Preenche quando você clica", text: "Na página de login, o HavenKeys oferece o que você salvou para aquele site e espera você escolher." },
      { icon: "lock", title: "Passkeys", text: "Crie passkeys e entre com elas, no computador e no celular." },
      { icon: "check", title: "Códigos de verificação inclusos", text: "Os códigos de duas etapas ficam junto da senha. Sem app autenticador à parte." },
      { icon: "laptop", title: "Funciona offline", text: "Cada dispositivo guarda sua própria cópia trancada, então suas senhas estão lá mesmo sem internet." },
      { icon: "monitor", title: "Em todos os seus dispositivos", text: "App de desktop, app Android e extensão para Chrome e Firefox, sempre sincronizados." },
      { icon: "download", title: "Traga suas senhas", text: "Importe do 1Password, Bitwarden, LastPass, KeePassXC, Chrome ou Firefox. Exporte quando quiser." },
    ],
    trustTitle: "Ninguém mais consegue abrir.",
    trustBody: "Seu cofre é trancado no seu dispositivo com duas coisas que só você tem: sua senha mestra e uma Secret Key que o HavenKeys cria para você. O que chega ao servidor já vai trancado, então quem o opera — nós incluídos — não consegue ler nenhuma senha.",
    trustLink: "Como funciona, em detalhes",
    stepsTitle: "Comece em três passos.",
    steps: [
      { title: "Instale o HavenKeys", text: "Baixe o app para Windows, macOS, Linux ou Android, e a extensão para Chrome ou Firefox." },
      { title: "Tenha sua conta", text: "Peça um convite para o nosso servidor, ou rode o seu." },
      { title: "Imprima seu Emergency Kit", text: "Ele guarda sua Secret Key. Deixe-o em lugar seguro: é com ele que você entra num dispositivo novo." },
    ],
    selfHostTitle: "Prefere seu próprio servidor?",
    selfHostBody: "Rode o HavenKeys num pequeno servidor seu ou no Railway. A instalação leva poucos comandos, e os backups diários já vêm incluídos.",
    selfHostCta: "Rode seu próprio servidor",
    closerTitle: /* keep current pt JSX */,
    closerLede: "Instale o app, peça um convite e adicione a extensão.",
    readSource: /* keep current pt text */,
```

Type `cards[].icon` as `IconName` (import the type from `../components/Icon` into en.tsx; declare `icon: "key" as IconName` or type the array) so Home can pass it to `<Icon>`.

- [ ] **Step 5: Messages — new `security`**

Replace `security` (its old content is in `developers`). en:

```tsx
  security: {
    heroTitle: "Built so that only you can open your vault.",
    heroLede: "What HavenKeys protects, what it can’t, and where to check for yourself.",
    protectsTitle: "What it protects",
    protects: [
      { title: "Your vault, wherever it’s stored", text: "Passwords, notes, codes and passkeys are locked on your device before they’re saved or sent. The server keeps a copy it has no key for." },
      { title: "A stolen server", text: "Someone who copies the server’s data still needs your master password and your Secret Key, which never leaves your devices." },
      { title: "Fake websites", text: "The extension offers a login only on the site it was saved for, and fills only after you click. Look-alike addresses don’t match." },
      { title: "Accidental leaks", text: "Passwords never show up in logs, notifications or window titles, and copied passwords are cleared from the clipboard." },
    ],
    limitsTitle: "What it can’t do",
    limits: [
      "Protect you from malware already running on your computer while your vault is unlocked.",
      "Bring back a vault if you lose your Emergency Kit and every device. There’s no account recovery: that’s the price of nobody else holding a key.",
      "Restore a server you run yourself that was lost without a backup.",
    ],
    auditTitle: "Honest about where it stands",
    auditBody: "HavenKeys is open source and its security design is written down in full, but it hasn’t had an independent audit yet.",
    deepTitle: "Want the details?",
    deepBody: "How your keys are made, what the extension may access, the attacks it’s tested against and the full threat model are on the Developers page.",
    deepCta: "Read the technical details",
  },
```

pt-BR:

```tsx
  security: {
    heroTitle: "Feito para que só você abra o seu cofre.",
    heroLede: "O que o HavenKeys protege, o que não consegue proteger, e onde conferir por conta própria.",
    protectsTitle: "O que ele protege",
    protects: [
      { title: "Seu cofre, onde quer que esteja", text: "Senhas, notas, códigos e passkeys são trancados no seu dispositivo antes de serem salvos ou enviados. O servidor guarda uma cópia que ele não tem como abrir." },
      { title: "Um servidor roubado", text: "Quem copiar os dados do servidor ainda precisa da sua senha mestra e da sua Secret Key, que nunca sai dos seus dispositivos." },
      { title: "Sites falsos", text: "A extensão só oferece um login no site para o qual ele foi salvo, e só preenche depois do seu clique. Endereços parecidos não valem." },
      { title: "Vazamentos acidentais", text: "Senhas nunca aparecem em logs, notificações ou títulos de janela, e senhas copiadas são apagadas da área de transferência." },
    ],
    limitsTitle: "O que ele não consegue fazer",
    limits: [
      "Proteger você de um malware já rodando no seu computador enquanto o cofre está aberto.",
      "Recuperar um cofre se você perder o Emergency Kit e todos os dispositivos. Não existe recuperação de conta: é o preço de ninguém mais ter uma chave.",
      "Restaurar um servidor seu que foi perdido sem backup.",
    ],
    auditTitle: "Honesto sobre onde está",
    auditBody: "O HavenKeys é código aberto e seu projeto de segurança está documentado por inteiro, mas ainda não passou por uma auditoria independente.",
    deepTitle: "Quer os detalhes?",
    deepBody: "Como suas chaves são criadas, o que a extensão pode acessar, os ataques contra os quais ele é testado e o modelo de ameaças completo estão na página Desenvolvedores.",
    deepCta: "Ler os detalhes técnicos",
  },
```

Remove from `home` (both locales) every key now unused; the `Messages` type will flag what Home/Developers still reference.

- [ ] **Step 6: Home.tsx**

Rewrite to (keep the hero stage with the three screenshots exactly as today):

```tsx
import { Link } from "react-router-dom";
import popup from "../assets/shots/popup.png";
import menuLogins from "../assets/shots/menu-logins.png";
import desktopVault from "../assets/shots/desktop-vault.png";
import { Guilloche } from "../components/Guilloche";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import { CHROME_STORE, FIREFOX_STORE, GITHUB, inviteHref } from "../lib/links";

export function Home() {
  const { t, path } = useI18n();
  const h = t.home;
  const invite = inviteHref(t.common.inviteSubject);
  return (
    <>
      <section className="hero">
        <Guilloche className="hero__rosette" />
        <div className="hero__copy">
          <h1>{h.heroTitle}</h1>
          <p className="hero__lede">{h.heroLede}</p>
          <div className="hero__actions">
            <Link to={path("/download")} className="btn btn--primary btn--lg">
              <Icon name="download" />
              {t.common.downloadCta}
            </Link>
            <a href={invite} className="btn btn--ghost btn--lg">
              {t.common.requestInvite}
            </a>
          </div>
          <p className="hero__meta">{h.heroMeta}</p>
        </div>
        {/* hero__stage: unchanged from the current file */}
      </section>

      <section className="section" id="browser">
        <div className="section__head">
          <h2>{h.cardsTitle}</h2>
          <p>{h.cardsLede}</p>
        </div>
        <ul className="cards">
          {h.cards.map((c) => (
            <li key={c.title} className="card">
              <Icon name={c.icon} size={22} />
              <h3>{c.title}</h3>
              <p>{c.text}</p>
            </li>
          ))}
        </ul>
      </section>

      <section className="section trust" id="journey">
        <div className="section__head">
          <h2>{h.trustTitle}</h2>
          <p>{h.trustBody}</p>
          <Link to={path("/developers")} className="text-link">
            {h.trustLink} <Icon name="arrowRight" size={15} />
          </Link>
        </div>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{h.stepsTitle}</h2>
        </div>
        <ol className="steps">
          {h.steps.map((s, i) => (
            <li key={s.title} className="step">
              <span className="step__n" aria-hidden="true">{i + 1}</span>
              <h3>{s.title}</h3>
              <p>{s.text}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className="section selfhost-teaser">
        <div className="section__head">
          <h2>{h.selfHostTitle}</h2>
          <p>{h.selfHostBody}</p>
          <Link to={path("/self-host")} className="btn btn--ghost">
            <Icon name="server" size={16} />
            {h.selfHostCta}
          </Link>
        </div>
      </section>

      <section className="closer">
        <Guilloche className="closer__rosette" />
        <h2>{h.closerTitle}</h2>
        <p>{h.closerLede}</p>
        <div className="hero__actions">
          <Link to={path("/download")} className="btn btn--primary btn--lg">
            <Icon name="download" />
            {t.common.downloadCta}
          </Link>
          <a href={invite} className="btn btn--ghost btn--lg">{t.common.requestInvite}</a>
          <a className="btn btn--ghost btn--lg" href={CHROME_STORE} target="_blank" rel="noopener noreferrer">
            <Icon name="external" size={15} />
            {t.common.addToChrome}
          </a>
          <a className="btn btn--ghost btn--lg" href={FIREFOX_STORE} target="_blank" rel="noopener noreferrer">
            <Icon name="external" size={15} />
            {t.common.addToFirefox}
          </a>
          <a href={GITHUB} className="btn btn--ghost btn--lg" target="_blank" rel="noreferrer">
            <Icon name="github" />
            {h.readSource}
          </a>
        </div>
      </section>
    </>
  );
}
```

The `/self-host` route arrives in Task 6; until then the link 404s in the app but tests don't follow it.

- [ ] **Step 7: Security.tsx**

```tsx
import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";

export function Security() {
  const { t, path } = useI18n();
  const s = t.security;
  return (
    <div className="security-plain">
      <section className="page-hero">
        <h1>{s.heroTitle}</h1>
        <p className="page-hero__lede">{s.heroLede}</p>
      </section>
      <section className="section">
        <div className="section__head"><h2>{s.protectsTitle}</h2></div>
        <ul className="cards">
          {s.protects.map((p) => (
            <li key={p.title} className="card">
              <Icon name="check" size={20} />
              <h3>{p.title}</h3>
              <p>{p.text}</p>
            </li>
          ))}
        </ul>
      </section>
      <section className="section">
        <div className="section__head"><h2>{s.limitsTitle}</h2></div>
        <ul className="plain-list">
          {s.limits.map((l) => (
            <li key={l}><Icon name="cross" size={16} /><span>{l}</span></li>
          ))}
        </ul>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.auditTitle}</h2>
          <p>{s.auditBody}</p>
          <p className="disclaimer">{t.common.disclaimer}</p>
        </div>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.deepTitle}</h2>
          <p>{s.deepBody}</p>
          <Link to={path("/developers")} className="btn btn--ghost">
            {s.deepCta} <Icon name="arrowRight" />
          </Link>
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 8: Nav and Footer**

Nav: remove the `#journey` and `#browser` anchors; after the Security `NavLink` add `<NavLink to={path("/self-host")} className="nav__hide-sm">{t.nav.selfHost}</NavLink>` and `<NavLink to={path("/developers")} className="nav__hide-sm">{t.nav.developers}</NavLink>`. Footer: add `<Link to={path("/self-host")}>{t.footer.selfHost}</Link>` and `<Link to={path("/developers")}>{t.footer.developers}</Link>` after Security.

- [ ] **Step 9: CSS**

Append to `apps/web/src/styles/global.css` (use the existing tokens; read the `:root` block and `.features`/`.setup__steps` rules first and match their spacing scale):

```css
/* Home and Security cards */
.cards {
  list-style: none;
  margin: 0 auto;
  padding: 0;
  display: grid;
  gap: 18px;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr));
  max-width: 1080px;
}
.card {
  padding: 24px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--raised);
}
.card svg { color: var(--brass-hi); }
.card h3 { margin: 12px 0 6px; font-size: 1.12rem; color: var(--text-strong); }
.card p { margin: 0; color: var(--muted-hi); }

/* Three steps */
.steps {
  list-style: none;
  margin: 0 auto;
  padding: 0;
  display: grid;
  gap: 18px;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 240px), 1fr));
  max-width: 1080px;
}
.step { position: relative; padding: 24px; border-top: 2px solid var(--brass-line); }
.step__n { font-family: var(--serif, Georgia, serif); font-size: 2rem; color: var(--brass); }
.step h3 { margin: 8px 0 6px; color: var(--text-strong); }
.step p { margin: 0; color: var(--muted-hi); }

/* Security: limits */
.plain-list { list-style: none; margin: 0 auto; padding: 0; max-width: 760px; display: grid; gap: 12px; }
.plain-list li { display: flex; gap: 10px; align-items: flex-start; color: var(--text); }
.plain-list svg { flex: none; margin-top: 4px; color: var(--danger); }
```

If a variable name differs (e.g. no `--serif`), use the one the stylesheet defines for Source Serif headings.

- [ ] **Step 10: Run and commit**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web build` → pass.

```bash
git add apps/web
git commit -m "feat(web): a Home and a Security page written for everyday users"
```

---

### Task 6: Self-host page, Download and legal edits

**Files:**
- Create: `apps/web/src/pages/SelfHost.tsx`, `apps/web/src/pages/SelfHost.test.tsx`
- Modify: `apps/web/src/App.tsx`, `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx`, `apps/web/src/styles/global.css`, `apps/web/src/App.test.tsx`

**Interfaces:**
- Consumes: `inviteHref`, `RAILWAY_TEMPLATE_URL`, `GITHUB` (Task 5); bundle file names and commands (Task 1); `docs/self-hosting.md` (Task 3).
- Produces: messages section `selfHost`; route `/self-host`.

- [ ] **Step 1: Failing tests**

`apps/web/src/pages/SelfHost.test.tsx`:

```tsx
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { SelfHost } from "./SelfHost";

const html = () => renderToString(<MemoryRouter><SelfHost /></MemoryRouter>);

describe("SelfHost", () => {
  it("shows the four Docker steps and the bundle download", () => {
    const h = html();
    expect(en.selfHost.steps).toHaveLength(4);
    expect(h).toContain("deploy/compose/$f");
    expect(h).toContain("./setup.sh");
    expect(h).toContain("admin new-account");
  });
  it("shows no Railway button until a template exists", () => {
    const h = html();
    expect(h).not.toContain("railway.com/deploy");
    expect(h.replace(/&#x27;|’/g, "'")).toContain(en.selfHost.railwaySoon.replace(/’/g, "'"));
  });
});
```

Add to `App.test.tsx`: `"/self-host"` to the page list of "still renders every existing page".

Run → FAIL.

- [ ] **Step 2: Messages `selfHost`** (en, after `security`)

```tsx
  selfHost: {
    title: "Run your own HavenKeys server",
    lede: "Keep your locked vault on a server you control. You need a domain, a small server and a few minutes.",
    needTitle: "What you need",
    needs: [
      "A small Linux server with Docker — about US$5 a month, or a computer at home that’s always on.",
      "A domain or subdomain you can point at it, like vault.example.com.",
      "Or, instead of both: a Railway account.",
    ],
    stepsTitle: "Four steps with Docker",
    steps: [
      { title: "Point your domain at the server", text: "Add an A record for your domain with the server’s IP address, and open ports 80 and 443.", code: "" },
      { title: "Download the bundle", text: "Six small files: the services, HTTPS, backups and the setup script.", code: "mkdir havenkeys && cd havenkeys\nfor f in compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh; do\n  curl -fsSLO \"https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/$f\"\ndone\nchmod +x setup.sh restore.sh" },
      { title: "Run the setup", text: "It asks for your domain and email, creates the secrets, and starts everything with HTTPS.", code: "./setup.sh" },
      { title: "Create your account", text: "It prints an invite once. Paste it into the HavenKeys app on your computer.", code: "docker compose exec server havenkeys-server admin new-account \\\n  --email you@example.com --server-url https://vault.example.com" },
    ],
    railwayTitle: "Or deploy on Railway",
    railwayBody: "No server to look after: Railway runs HavenKeys and its database for you, for about US$5 a month.",
    railwayCta: "Deploy on Railway",
    railwaySoon: "The one-click Railway template is coming soon. Until then, the guide has the Railway steps.",
    backupsTitle: "Backups are included",
    backupsBody: "The bundle saves a copy of the database every night and keeps the last 14 days in a backups folder. Copy that folder somewhere else regularly, and try a restore once so you know it works.",
    chargeTitle: "You’re in charge",
    chargeBody: "When you run the server, keeping it online and backed up is up to you. HavenKeys can’t recover a vault from a server that was lost without a backup.",
    guideCta: "Read the full guide",
    inviteInstead: "Rather not run a server? Request an invite to ours.",
  },
```

pt-BR:

```tsx
  selfHost: {
    title: "Rode seu próprio servidor HavenKeys",
    lede: "Guarde seu cofre trancado num servidor que você controla. Você precisa de um domínio, um pequeno servidor e alguns minutos.",
    needTitle: "O que você precisa",
    needs: [
      "Um pequeno servidor Linux com Docker — cerca de US$5 por mês, ou um computador em casa sempre ligado.",
      "Um domínio ou subdomínio que você possa apontar para ele, como cofre.exemplo.com.",
      "Ou, no lugar dos dois: uma conta no Railway.",
    ],
    stepsTitle: "Quatro passos com Docker",
    steps: [
      { title: "Aponte seu domínio para o servidor", text: "Crie um registro A do seu domínio com o IP do servidor, e libere as portas 80 e 443.", code: "" },
      { title: "Baixe o pacote", text: "Seis arquivos pequenos: os serviços, o HTTPS, os backups e o script de instalação.", code: /* same as en */ },
      { title: "Rode a instalação", text: "Ele pede seu domínio e e-mail, cria os segredos e sobe tudo com HTTPS.", code: "./setup.sh" },
      { title: "Crie sua conta", text: "Ele mostra um convite uma única vez. Cole-o no app HavenKeys do seu computador.", code: "docker compose exec server havenkeys-server admin new-account \\\n  --email voce@exemplo.com --server-url https://cofre.exemplo.com" },
    ],
    railwayTitle: "Ou publique no Railway",
    railwayBody: "Sem servidor para cuidar: o Railway roda o HavenKeys e o banco de dados para você, por cerca de US$5 por mês.",
    railwayCta: "Publicar no Railway",
    railwaySoon: "O modelo de um clique para o Railway chega em breve. Até lá, o guia tem os passos para o Railway.",
    backupsTitle: "Backups inclusos",
    backupsBody: "O pacote salva uma cópia do banco de dados toda noite e guarda os últimos 14 dias numa pasta de backups. Copie essa pasta para outro lugar com frequência, e teste uma restauração uma vez para saber que funciona.",
    chargeTitle: "A responsabilidade é sua",
    chargeBody: "Quando você roda o servidor, mantê-lo no ar e com backup é com você. O HavenKeys não recupera um cofre de um servidor perdido sem backup.",
    guideCta: "Ler o guia completo",
    inviteInstead: "Prefere não rodar um servidor? Peça um convite para o nosso.",
  },
```

- [ ] **Step 3: `SelfHost.tsx`**

```tsx
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import { GITHUB, inviteHref, RAILWAY_TEMPLATE_URL } from "../lib/links";

const GUIDE = `${GITHUB}/blob/main/docs/self-hosting.md`;

export function SelfHost() {
  const { t } = useI18n();
  const s = t.selfHost;
  return (
    <div className="selfhost">
      <section className="page-hero">
        <h1>{s.title}</h1>
        <p className="page-hero__lede">{s.lede}</p>
      </section>

      <section className="section">
        <div className="section__head"><h2>{s.needTitle}</h2></div>
        <ul className="plain-list plain-list--ok">
          {s.needs.map((n) => (
            <li key={n}><Icon name="check" size={16} /><span>{n}</span></li>
          ))}
        </ul>
      </section>

      <section className="section">
        <div className="section__head"><h2>{s.stepsTitle}</h2></div>
        <ol className="selfhost__steps">
          {s.steps.map((step, i) => (
            <li key={step.title}>
              <span className="step__n" aria-hidden="true">{i + 1}</span>
              <div>
                <h3>{step.title}</h3>
                <p>{step.text}</p>
                {step.code && <pre className="code"><code>{step.code}</code></pre>}
              </div>
            </li>
          ))}
        </ol>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.railwayTitle}</h2>
          <p>{s.railwayBody}</p>
          {RAILWAY_TEMPLATE_URL ? (
            <a className="btn btn--primary" href={RAILWAY_TEMPLATE_URL} target="_blank" rel="noopener noreferrer">
              {s.railwayCta} <Icon name="external" size={15} />
            </a>
          ) : (
            <p className="muted">{s.railwaySoon}</p>
          )}
        </div>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.backupsTitle}</h2>
          <p>{s.backupsBody}</p>
          <h2>{s.chargeTitle}</h2>
          <p>{s.chargeBody}</p>
          <div className="hero__actions">
            <a className="btn btn--ghost" href={GUIDE} target="_blank" rel="noreferrer">
              {s.guideCta} <Icon name="external" size={15} />
            </a>
            <a className="btn btn--ghost" href={inviteHref(t.common.inviteSubject)}>{s.inviteInstead}</a>
          </div>
        </div>
      </section>
    </div>
  );
}
```

CSS (append):

```css
.selfhost__steps { list-style: none; margin: 0 auto; padding: 0; max-width: 820px; display: grid; gap: 28px; }
.selfhost__steps > li { display: grid; grid-template-columns: 44px minmax(0, 1fr); gap: 12px; }
.selfhost__steps h3 { margin: 4px 0 6px; color: var(--text-strong); }
.selfhost__steps p { margin: 0 0 10px; color: var(--muted-hi); }
.code { margin: 0; padding: 14px 16px; overflow-x: auto; border: 1px solid var(--line); border-radius: 10px; background: var(--ink); font-family: "JetBrains Mono", monospace; font-size: 0.86rem; line-height: 1.55; color: var(--text); white-space: pre; }
.plain-list--ok svg { color: var(--ok); }
.muted { color: var(--muted); }
```

(`.code` scrolls horizontally inside itself so the page never overflows on a phone. Check whether `.muted` already exists before adding it.)

Route in `App.tsx`: `{ path: "self-host", element: <SelfHost /> }`.

- [ ] **Step 4: Download — "Get an account" step**

Replace the first item of `download.steps` (en) — "You’ll need a server" — with:

```tsx
      {
        title: "Get an account",
        body: (
          <>
            HavenKeys keeps your locked vault on a server. Request an invite to ours, or{" "}
            <Link to="/self-host">run your own server</Link>. Then print your Emergency Kit: it’s how you
            sign in on a new device.
          </>
        ),
        actions: (
          <a className="btn btn--ghost btn--sm" href="mailto:invite@havenkeys.net?subject=HavenKeys%20invite%20request">
            Request an invite
          </a>
        ),
      },
```

pt-BR:

```tsx
      {
        title: "Tenha sua conta",
        body: (
          <>
            O HavenKeys guarda seu cofre trancado num servidor. Peça um convite para o nosso, ou{" "}
            <Link to="/pt-br/self-host">rode seu próprio servidor</Link>. Depois imprima seu Emergency Kit:
            é com ele que você entra num dispositivo novo.
          </>
        ),
        actions: (
          <a className="btn btn--ghost btn--sm" href="mailto:invite@havenkeys.net?subject=Pedido%20de%20convite%20HavenKeys">
            Pedir um convite
          </a>
        ),
      },
```

(Messages can't call `inviteHref` without importing it; import `inviteHref` from `../lib/links` in both message files and use `inviteHref("HavenKeys invite request")` / `inviteHref("Pedido de convite HavenKeys")` instead of the literal hrefs.)

- [ ] **Step 5: Legal and privacy (draft for owner approval)**

Terms (en) — replace the "No service, no account" `<h2>` and paragraph with:

```tsx
        <h2>Our server and yours</h2>
        <p>
          The author runs a <code>havenkeys-server</code> for people he invites, on a best-effort basis:
          there is no subscription, no service-level agreement and no support obligation, and the service
          may change or end with notice. Anyone can instead run their own server; downloading the
          software creates no account with us.
        </p>
```

Use "they invite" if the existing Terms/Privacy text refers to the author neutrally (check how the Privacy page names "SAMUEL DA SILVA ROCHA" and mirror it). In the "Self-hosting" paragraph, add `<Ext href={`${DOCS}self-hosting.md`}>the self-hosting guide</Ext>` before the deployment-guide link ("See the self-hosting guide and the deployment guide, particularly…").

pt-BR — the same, translated:

```tsx
        <h2>Nosso servidor e o seu</h2>
        <p>
          O autor opera um <code>havenkeys-server</code> para as pessoas que convida, na base do melhor
          esforço: não há assinatura, acordo de nível de serviço nem obrigação de suporte, e o serviço pode
          mudar ou acabar mediante aviso. Qualquer pessoa pode, em vez disso, rodar o próprio servidor;
          baixar o software não cria conta nenhuma conosco.
        </p>
```

Privacy (both): after the paragraph about the site setting no cookies, add one paragraph — en: "If you email invite@havenkeys.net, the operator receives your email address and message, uses them only to answer you and send an invite, and deletes them when you ask." pt-BR: "Se você escrever para invite@havenkeys.net, o operador recebe seu e-mail e sua mensagem, usa-os apenas para responder e enviar o convite, e os apaga quando você pedir."

- [ ] **Step 6: Run and commit**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web build` → pass.

```bash
git add apps/web
git commit -m "feat(web): a Self-host page, an account step on Download, and corrected terms"
```

---

### Task 7: Visual check of the site

**Files:**
- Create: `tools/ui-check/web.mjs`
- Modify: `tools/ui-check/run.mjs`

**Interfaces:**
- Consumes: the built site (`apps/web/dist`), all routes from Tasks 4–6.

- [ ] **Step 1: Scenarios**

`tools/ui-check/web.mjs`:

```js
// Website pages for ui-check: every page in both languages, at a phone and a
// desktop width, full page.
export const webPages = ["/", "/download", "/security", "/self-host", "/developers", "/privacy", "/terms", "/delete-account"];
export const webSizes = [
  { name: "phone", width: 390, height: 844 },
  { name: "desktop", width: 1280, height: 800 },
];
export function webPath(page, locale) {
  if (locale === "en") return page;
  return page === "/" ? "/pt-br" : `/pt-br${page}`;
}
```

- [ ] **Step 2: Wire it into `run.mjs`**

1. Usage comment: `pnpm ui:check --app=web   the website`.
2. Imports: `import { webPages, webSizes, webPath } from "./web.mjs";` and `const WEB_DIST = join(root, "apps/web/dist");`.
3. Build: change the two build lines so `--app=web` builds only the site, and a full run builds all three:

```js
if (build) {
  if (!appFilter || appFilter === "extension") run("pnpm", ["--filter", "@havenkeys/extension", "build"]);
  if (!appFilter || appFilter === "desktop") run("pnpm", ["--filter", "@havenkeys/desktop", "exec", "vite", "build", "--logLevel", "warn"]);
  if (!appFilter || appFilter === "web") run("pnpm", ["--filter", "@havenkeys/web", "exec", "vite", "build", "--logLevel", "warn"]);
}
```

4. Change the two existing app conditions from `appFilter !== "desktop"` / `appFilter !== "extension"` to `(!appFilter || appFilter === "extension")` / `(!appFilter || appFilter === "desktop")`.
5. A second static server for the site with SPA fallback (unknown paths → `index.html`), after the first server:

```js
const webServer = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  let file = normalize(join(WEB_DIST, decodeURIComponent(url.pathname)));
  if (!file.startsWith(WEB_DIST)) {
    res.writeHead(403).end();
    return;
  }
  try {
    if (!(await stat(file)).isFile()) throw new Error("dir");
  } catch {
    file = join(WEB_DIST, "index.html");
  }
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
  res.end(await readFile(file));
});
await new Promise((resolve) => webServer.listen(0, "127.0.0.1", resolve));
const webUrl = `http://127.0.0.1:${webServer.address().port}`;
```

6. The web loop, before `await browser.close()`:

```js
if (!appFilter || appFilter === "web") {
  for (const page of webPages) {
    const name = page === "/" ? "home" : page.slice(1);
    if (only && !name.includes(only)) continue;
    for (const locale of LOCALES) {
      for (const size of webSizes) {
        const { context, page: tab } = await newPage(locale, "dark");
        const label = `web/${locale}/${size.name}/${name}`;
        tab.__label = label;
        await tab.setViewportSize({ width: size.width, height: size.height });
        // Pin the language so the homepage does not redirect.
        await tab.addInitScript((l) => localStorage.setItem("hk-locale", l), locale);
        await tab.goto(`${webUrl}${webPath(page, locale)}`);
        await tab.evaluate(() => document.fonts.ready);
        await tab.addStyleTag({ content: STILL });
        await tab.waitForTimeout(250);
        const found = await tab.evaluate(overflowCheck);
        for (const f of found) failures.push({ label, ...f });
        const file = join(OUT, "web", locale, size.name, `${name}.png`);
        await mkdir(dirname(file), { recursive: true });
        await tab.screenshot({ path: file, fullPage: true });
        shots++;
        await context.close();
      }
    }
  }
}
```

Close `webServer` next to `server.close()`.

- [ ] **Step 3: Run and look**

Run: `pnpm ui:check --app=web`. Expected: no overflow/clipped-text problems. Fix any reported problem in the page CSS/markup (not by weakening the check). Then open and look at, at least: `ui-check-output/web/en/phone/home.png`, `web/en/desktop/home.png`, `web/pt-BR/phone/self-host.png`, `web/en/desktop/developers.png`, `web/en/phone/security.png`. Check: hero readable, six cards in a grid (one column on phone), steps numbered, code blocks scroll inside themselves on phone, Developers shows the moved sections in order. List the files you viewed in the report.

- [ ] **Step 4: Commit**

```bash
git add tools/ui-check apps/web
git commit -m "test(ui-check): the website, in both languages, at phone and desktop widths"
```

---

### Task 8: Final verification and site docs

**Files:**
- Modify: `docs/website.md` (pages list, if it has one: `grep -n "Security\|pages" docs/website.md`)

- [ ] **Step 1:** Update `docs/website.md`'s description of the pages (Home, Download, Security, Self-host, Developers, legal) and note that `RAILWAY_TEMPLATE_URL` in `apps/web/src/lib/links.ts` turns on the Railway button.
- [ ] **Step 2:** Run: `pnpm --filter @havenkeys/web test`, `pnpm --filter @havenkeys/web typecheck`, `pnpm --filter @havenkeys/web build`, `pnpm ui:check --app=web --no-build`, `sh -n deploy/compose/*.sh`, `docker compose -f deploy/compose/compose.yaml --env-file deploy/compose/.env.example config -q` (expect it to fail on the empty required variables — that proves the `:?` guards; then run it once more with a temp env file that has values, expect success).
- [ ] **Step 3: Commit**

```bash
git add docs/website.md
git commit -m "docs(website): the new pages and the Railway switch"
```

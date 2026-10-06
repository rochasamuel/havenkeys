# Website refresh and easy self-hosting — Design

Date: 2026-10-06. Status: approved in conversation.

> This software has not undergone an independent security audit.

## 1. Goal

Two things:

1. **The website speaks to everyday people first.** Today havenkeys.net
   leads with the Rust core, a fourteen-item feature list, the Secret Key's
   bit count and the threat-model ledger. A visitor who just wants a
   password manager should instead see what it does, how to start, and why
   nobody else can open their vault — in plain words. The technical
   material moves to a **Developers** page, nothing is deleted.
2. **Running your own server is a few commands, not a project.** Today a
   self-hoster compiles the Rust server, provisions Postgres, arranges TLS
   and follows a Railway-specific guide. They should be able to point a
   domain at a small server, run one script, and have HTTPS, the database
   and nightly backups working; or press a "Deploy on Railway" button.

No server, client or protocol code changes. The security model is unchanged.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Who the homepage speaks to | **Everyday users first**; self-hosting is a secondary path | Self-hosters first; two equal paths |
| How an everyday user gets an account | **Request an invite** by email; the operator creates it with `admin new-account` | Open sign-up (needs new server endpoints; a separate project); no hosted path |
| Invite channel | **`mailto:invite@havenkeys.net`** with a prefilled subject (alias forwarded by ImprovMX, set up 2026-10-06) | Web form (stores personal data, privacy change); GitHub issues (public) |
| Self-hosting paths | **Docker Compose bundle** (prebuilt image, Postgres, Caddy for HTTPS, nightly backups, setup script) and a **Railway template** | Home server via Tailscale (later); single binary + SQLite (server change) |
| Where the full guide lives | `docs/self-hosting.md` in the repository; the site's `/self-host` page is a friendly summary that links to it | Full guide on the site (two copies drift) |
| Feature list | **Main features only**, about six, in plain words | The full list (it stays on Developers, condensed) |

## 3. Site structure

Routes keep the existing English / `/pt-br` pair for every page.

| Page | Route | For | Content |
|---|---|---|---|
| Home | `/` | everyone | §4 |
| Download | `/download` | everyone | unchanged downloads, plus a **Getting started** box (§5) |
| Self-host | `/self-host` (new) | people who want their own server | §6 |
| Developers | `/developers` (new) | technical readers | §7 |
| Security | `/security` | everyone | rewritten in plain language (§8) |
| Privacy, Terms, Delete account | unchanged routes | everyone | corrections only (§9) |

**Nav:** Download (button), Security, Self-host, Developers. **Footer:**
adds Self-host and Developers. Old in-page anchors on Home (`#journey`,
`#browser`) keep resolving: `#journey` and the ledger move to Developers,
so `/#journey` scrolls to the new "How it works" teaser on Home, which links
on. No anchor 404s.

## 4. Home

Visual language unchanged (`apps/web/DESIGN.md`: forest green, brass,
Source Serif headings, the paper chapter). Copy rewritten, sections:

1. **Hero.** Title along the lines of "Your passwords, locked — and only
   yours." One-sentence lede: a password manager for your computer, phone
   and browser, where nobody but you can open the vault. Buttons:
   **Download** (primary) and **Request an invite** (ghost, `mailto`).
   Meta line: free and open source · Windows, macOS, Linux, Android ·
   Chrome and Firefox. The existing desktop/popup/menu screenshots stay.
2. **What it does** — six feature cards, icon + short title + one line:
   * **Fills logins when you click** — the browser menu offers what's saved
     for that site and waits for you.
   * **Passkeys** — create and use passkeys on websites.
   * **One-time codes built in** — no separate authenticator app needed.
   * **Works offline** — every device keeps an encrypted copy.
   * **On all your devices** — desktop, Android and the browser extension,
     kept in sync.
   * **Bring your passwords** — import from 1Password, Bitwarden, LastPass,
     KeePassXC, Chrome or Firefox; export any time.
   The cards are checked against what ships (§11 test).
3. **Nobody else can open it.** One friendly paragraph: the vault is
   locked on your device with your master password and your Secret Key;
   the server only ever stores locked data. Link: "How it works, in
   detail →" to `/developers`.
4. **Start in three steps.** Install → get an account (request an invite,
   or run your own server) → print your Emergency Kit. Each step one line.
5. **Prefer your own server?** Short teaser: run it on a small server or on
   Railway, with backups included. Button to `/self-host`.
6. **Closer.** Existing "Bring your passwords home" closer, with Download
   and Request an invite.

The Journey animation and the browser showcase move to Developers (they
explain the mechanism); the paper/Emergency Kit chapter moves to Developers
with its full text, and Home keeps the kit only as step 3's illustration
if it fits without crowding.

## 5. Download: Getting started box

Above or beside the download cards:

> **Getting started** 1. Install the app (and the extension). 2. Sign in:
> with an invite from us — [Request an invite] — or with your own server
> ([Self-host]). 3. Print your Emergency Kit and keep it safe.

## 6. Self-host page (`/self-host`)

Friendly, short, honest:

* **What you need:** a small Linux server (about US$5/month, or a computer
  at home that's always on) with Docker, and a domain name you control —
  *or* a Railway account.
* **Four steps** (Compose): point a domain at the server → download the
  bundle → run `./setup.sh` → create your account with one command. Each
  step a one-liner with the command shown.
* **Deploy on Railway** button (rendered only when the template URL is
  configured; until then a "coming soon" line and the link to the guide).
* **Backups:** one paragraph — the bundle backs up the database every night
  and keeps 14 days; copy them off the server; test a restore once.
* **Honesty box:** you are responsible for your server's uptime and
  backups; HavenKeys can't recover a vault from a lost server.
* Link to the full guide `docs/self-hosting.md` on GitHub.

## 7. Developers page (`/developers`)

Everything technical that leaves Home, plus pointers:

* **How it works:** the Journey animation and its text; the key hierarchy
  in brief (master password + Secret Key → Argon2id → keys; the Rust core
  does all cryptography; the server holds ciphertext only).
* **The browser extension in detail:** the browser showcase and the
  condensed extension feature list.
* **Two secrets:** the paper chapter (Secret Key, Emergency Kit, no account
  recovery) with its current text.
* **What it defends, what it doesn't:** the ledger, unchanged.
* **Architecture and docs:** links to `docs/architecture.md`,
  `security-model.md`, `threat-model.md`, `crypto.md`,
  `security-review.md`, `native-messaging.md`, `self-hosting.md`,
  `deployment.md`.
* **Build from source:** pointer to `docs/development.md`; licence; GitHub.

## 8. Security page

Rewritten in plain language: what's protected (your vault is encrypted on
your device; the server can't read it; the extension fills only after you
click and only on the right site), what isn't (malware on your computer
while unlocked; a lost server without backups), and that there has been no
independent audit. Deep detail links to Developers and the docs.

## 9. Legal and privacy corrections

* **Terms, "No service, no account":** today says "We do not operate a
  hosted version". Correct it to say the author runs a server for invited
  users, best effort, with no SLA or subscription, and that self-hosters
  run their own. The new text is a draft for the owner to approve before
  merge — it is legal copy.
* **Terms, "Self-hosting":** link `docs/self-hosting.md` alongside the
  deployment guide.
* **Privacy:** add one sentence that emailing `invite@havenkeys.net`
  shares your email address and message with the operator, used only to
  answer and send the invite. The site itself still stores nothing.

## 10. Self-hosting deliverables

### 10.1 Prebuilt image

`.github/workflows/server-image.yml`: on tags `server-v*` (and manual
dispatch), build the root `Dockerfile` for `linux/amd64` and `linux/arm64`
with Buildx and push to `ghcr.io/rochasamuel/havenkeys-server` as
`<version>` (tag without the `server-v` prefix) and `latest`. Uses the
built-in `GITHUB_TOKEN` with `packages: write`; no other secrets. The
package is made public once by the owner in GitHub's package settings
(documented). Build cache via the GitHub Actions cache.

### 10.2 Compose bundle (`deploy/compose/`)

* `compose.yaml`:
  * `server`: `ghcr.io/rochasamuel/havenkeys-server:${HAVENKEYS_VERSION:-latest}`,
    env `DATABASE_URL=postgres://havenkeys:${POSTGRES_PASSWORD}@db:5432/havenkeys?sslmode=disable`
    (private Docker network, never published), `SERVER_SECRET`,
    `HAVENKEYS_TRUST_FORWARDED_FOR=1` (Caddy is the only peer and appends
    the client address; the deployment guide's caveat about this header
    is linked), restart `unless-stopped`, healthcheck on `/v1/health`, no
    published ports.
  * `db`: `postgres:16-alpine`, named volume, healthcheck `pg_isready`,
    no published ports.
  * `caddy`: `caddy:2-alpine`, ports 80 and 443, named volumes for its
    data and config, `Caddyfile` mounted read-only.
  * `backup`: `postgres:16-alpine` running a small loop: at 03:00 local
    time `pg_dump -Fc` to `./backups/havenkeys-YYYY-MM-DD.dump`, delete
    dumps older than `BACKUP_KEEP_DAYS` (default 14). Bind-mounted
    `./backups` so the owner can copy them off the host.
* `Caddyfile`: `{$HAVENKEYS_DOMAIN}` with `email {$ACME_EMAIL}`,
  `reverse_proxy server:8080`, security headers
  (`Strict-Transport-Security`), nothing else. HTTP redirects to HTTPS
  (Caddy's default).
* `.env.example`: `HAVENKEYS_DOMAIN`, `ACME_EMAIL`, `SERVER_SECRET`,
  `POSTGRES_PASSWORD`, `HAVENKEYS_VERSION`, `BACKUP_KEEP_DAYS`, each with a
  one-line comment.
* `setup.sh` (POSIX sh, idempotent):
  1. checks `docker` and `docker compose` exist;
  2. if `.env` is missing: asks for domain and email, generates
     `SERVER_SECRET` (`openssl rand -base64 32`, or `head -c 32
     /dev/urandom | base64` without openssl) and a 32-char
     `POSTGRES_PASSWORD`, writes `.env` with mode 600; never echoes the
     secrets;
  3. warns if the domain does not resolve to this machine's public
     address (best effort, non-fatal);
  4. `docker compose pull && docker compose up -d`;
  5. waits up to 2 minutes for `https://<domain>/v1/health` to return
     200, then prints the next step (create the first account) or the
     `docker compose logs` command to diagnose.
* `restore.sh`: restores a chosen dump into the `db` service (stops the
  server, `pg_restore --clean`, starts it), with a confirmation prompt.
* `README.md` in the folder: three lines pointing to `docs/self-hosting.md`.

Users get the bundle without cloning the repo:
`curl -fsSL https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/{compose.yaml,Caddyfile,.env.example,setup.sh,restore.sh}`
(exact command in the guide), or by downloading the folder from a release.

### 10.3 Railway template

`deploy/railway/README.md`: the exact dashboard steps for the owner to
publish a template from the existing project (Postgres + the server built
from this repo's `Dockerfile` with `railway.json`, variables
`DATABASE_URL=${{Postgres.DATABASE_URL}}?sslmode=disable`,
`SERVER_SECRET=${{secret(44)}}`-style generated value as Railway's template
editor offers, `HAVENKEYS_TRUST_FORWARDED_FOR=1`, `PORT=8080`, a generated
domain on port 8080). Once published, the owner puts the template URL in
the site config (`apps/web/src/lib/links.ts`, `RAILWAY_TEMPLATE_URL`,
empty by default) and the button appears.

### 10.4 `docs/self-hosting.md`

For a non-expert. Sections: what you're setting up (one diagram-free
paragraph); what you need; option A — your own server with Docker
(DNS record, firewall ports 80/443, download bundle, `./setup.sh`, check
health); option B — Railway; create your first account
(`docker compose exec server havenkeys-server admin new-account --email
you@example.com`, what the invite string is, where to paste it in the
app); connect your apps (the server URL is `https://<domain>`); backups
and a restore test (blocking, as in `deployment.md` §5); upgrading
(`docker compose pull && docker compose up -d`; pin
`HAVENKEYS_VERSION` to avoid surprises); troubleshooting (certificate not
issued → DNS/ports; health fails → `docker compose logs server`; "waiting
for the database"). Links `deployment.md` for environment variable
reference and advanced notes, and repeats its security warnings (TLS
only; the `X-Forwarded-For` caveat; no audit).

`docs/deployment.md` gains a pointer at the top: "Most people want
`self-hosting.md`."

## 11. Testing

* Site: existing vitest suites keep passing; new tests for the router
  (every page exists in both locales; `/self-host`, `/developers` render);
  a test that every Home feature card has an entry in both locales; the
  `mailto:` link has the address and an encoded subject; the Railway
  button is absent when the URL is empty. `pnpm --filter @havenkeys/web
  build` passes.
* Visual: the web app has no ui-check today; screenshots of the new pages
  at phone and desktop widths, both themes if the site has them, via
  Playwright (`playwright` is already a dev dependency), inspected before
  merge.
* Compose: `docker compose config` validates with a sample `.env`;
  `shellcheck` on `setup.sh` and `restore.sh` if available; a local smoke
  run with a locally built image (`HAVENKEYS_IMAGE` override) and Caddy's
  `tls internal` for a `localhost` domain, checking `/v1/health` and that
  the backup service writes a dump when triggered manually. If Docker is
  unavailable in the build environment, say so and leave the smoke run to
  the owner with exact commands.
* Workflow: `actionlint` if available; otherwise a careful read. First
  real run is the owner pushing a `server-v0.17.0` tag.

## 12. Out of scope

* Open sign-up, any server or client change.
* Home-server-without-a-domain (Tailscale) guide.
* A web form for invites.
* Translating `docs/self-hosting.md` (English only, like the other docs).

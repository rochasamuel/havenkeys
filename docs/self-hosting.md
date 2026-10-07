# Run your own HavenKeys server

> This software has not undergone an independent security audit.

HavenKeys keeps your encrypted vault on a server you control. The server
only stores ciphertext and cannot read your passwords. Your desktop and
Android apps sync through it. The server does see routing metadata
(accounts, device names, timing); see [server-sync.md](server-sync.md). This guide gets one running.

## What you need

**Option A: your own server with Docker.**

* A Linux server with Docker ([install guide](https://docs.docker.com/engine/install/)),
  Docker Compose v2 and `curl`. Your user must be able to run Docker (be in
  the `docker` group, or use `sudo`). Membership of the `docker` group is
  equivalent to root access on that machine, so add only users you trust.
* 1 GB of RAM is
  enough. Most providers charge about US$5 a month. An always-on
  computer at home also works if it is reachable from the internet.
* A domain or subdomain you control, such as `vault.example.com`.

**Option B: Railway.** A Railway account. No server to manage.

## Option A: your own server with Docker

### 1. Point your domain at the server

At your DNS provider, create an `A` record for the name you chose (for
example `vault.example.com`) pointing to the server's IP address. If the
server has IPv6, add an `AAAA` record too.

Open ports 80 and 443. For example, with `ufw`:

```sh
sudo ufw allow 80,443/tcp
```

### 2. Download the bundle

```sh
mkdir havenkeys && cd havenkeys
for f in compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh; do
  curl -fsSLO "https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/$f"
done
chmod +x setup.sh restore.sh backup.sh
```

Run every `docker compose` command in this guide from inside the
`havenkeys` folder.

### 3. Run the setup

First wait until your domain resolves to the server. If your DNS provider
proxies traffic (for example Cloudflare's orange cloud), set the record to
DNS-only and leave it that way permanently, not just while the certificate
is issued. The bundle trusts the address Caddy reports for each client,
and a proxy in front would break that (see Security notes).

```sh
./setup.sh
```

It checks that Docker, Docker Compose v2 and `curl` are installed. Then it
asks for two things:

* the domain (for example `vault.example.com`);
* an email address, which Let's Encrypt uses for certificate notices.

It writes a file called `.env` (readable only by you) with those answers
and two random secrets: `SERVER_SECRET` and the database password. If
`.env` already exists, setup keeps it. It also creates a private `backups`
folder, starts everything, and waits up to two minutes for
`https://<your domain>/v1/health` to answer.

**Back up `.env` somewhere safe, away from this server.** With it you can
rebuild the same server quickly. Database dumps do not need it: they restore
into a fresh install with its own new secrets.

Success looks like this:

```text
HavenKeys is running at https://vault.example.com
```

followed by the command for the next step.

### 4. Create your account

```sh
docker compose exec server havenkeys-server admin new-account \
  --email you@example.com --server-url https://vault.example.com
```

It prints an invite starting with `HKINV1-`. It is shown once, works once,
and expires in 7 days. Paste it into the first screen of the HavenKeys
desktop app, then print the Recovery Sheet the app offers.

Other admin commands:

```sh
docker compose exec server havenkeys-server admin list-accounts
docker compose exec server havenkeys-server admin delete-account --email you@example.com
```

`delete-account` cannot be undone.

## Option B: Railway

If the template has been published, the Self-host page on havenkeys.net has
the deploy button. Otherwise follow the manual steps in
[deployment.md](deployment.md) section 3.

Railway costs about US$5/month. **Turn on Railway's database backups**
before storing real passwords ([deployment.md](deployment.md) section 5).

## Connect your apps

Your server address is `https://<your domain>`.

* **Desktop:** on the first-run screen, paste the invite from step 4.
* **Android:** scan the Recovery Sheet, or sign in with your account.
* **Browser extension:** it talks to the desktop app, so install the
  desktop app first.

## Backups (do this before storing real passwords)

The server is the authoritative copy of your vault. No backup, no recovery.

The bundle dumps the database every night at 03:00 (time zone from `TZ` in
`.env`) into `./backups`, and keeps the last 14 days (`BACKUP_KEEP_DAYS`).

* A failed dump never replaces or deletes a good one. Dumps older than the
  keep period are deleted only after a new dump succeeds.
* The nightly run is skipped if the backup container is down at 03:00.
* The nightly run only logs when it fails. Check now and then:

  ```sh
  docker compose logs backup
  ```

Take a backup right now:

```sh
docker compose exec backup havenkeys-backup now
```

Copy the backups to another machine. From the `havenkeys` folder, for example:

```sh
rsync -a backups/ user@other-host:havenkeys-backups/
```

**Restore drill.** A backup you have never restored is a guess. Do the drill
on a second machine only. A separate folder on the same host is the same
Compose project (`name: havenkeys`), so setting up or restoring there would
hit your live database.

1. On the second machine, download the bundle (step 2).
2. Run a fresh `./setup.sh` with a test subdomain, for example
   `drill.example.com`, pointed at that machine. You do not need your live
   `.env`: a dump restores into an install with new secrets.
3. Copy a dump into its `backups/` folder.
4. Restore it:

   ```sh
   ./restore.sh backups/havenkeys-YYYY-MM-DD.dump
   ```

   It asks you to type `yes`, stops the server, restores the dump, and
   starts the server again.
5. Check the result: run
   `docker compose exec server havenkeys-server admin list-accounts`, and
   point a HavenKeys app at the test address to check your items.

**Never run `restore.sh` on your live server unless you mean to replace its data.**
If a restore fails, the script says the server is stopped; fix the problem
and run `docker compose start server`.

If you do restore over a live server, devices that had synced past the
backup notice at their next sync that the server went back in time. Each
one downloads the whole vault again and drops what the server no longer
has: changes made after the backup are lost on every device, as they are on
the server.

## Upgrading

Take a backup first (see above). Then:

```sh
docker compose pull && docker compose up -d
```

Database migrations run when the server starts. Setup wrote
`HAVENKEYS_VERSION=latest` in `.env`. To choose when to upgrade, change it
to a release such as `0.17.0`.

## Troubleshooting

**The certificate is not issued.** Usually DNS does not point to this server
yet, ports 80 and 443 are closed, or another web server is using them.
Look at the log:

```sh
docker compose logs caddy
```

**`/v1/health` does not answer.**

```sh
docker compose logs server
```

If it says "waiting for the database", the `db` service is not healthy:

```sh
docker compose logs db
```

**I lost the invite.** First check whether the account was ever activated:

```sh
docker compose exec server havenkeys-server admin list-accounts
```

If it never was, run `delete-account` for that email and create the account
again with `new-account`. Deleting an account that holds data erases that data.

## Security notes

* HTTPS only. The apps refuse plain `http`.
* `HAVENKEYS_TRUST_FORWARDED_FOR=1` is set because Caddy is the only peer
  and it replaces `X-Forwarded-For` with the real client address (it ignores
  incoming values from untrusted peers), so the server can trust it. Do not
  add `trusted_proxies` to the Caddyfile or put a CDN proxy in front without
  reading [deployment.md](deployment.md) section 2 first.
* Keep the host's operating system and Docker up to date.
* Never expose Postgres to the internet. The bundle does not publish its
  port.
* This software has not undergone an independent security audit.

## Reference

[deployment.md](deployment.md) lists every environment variable and has
platform notes.

## For maintainers

The server image is published to `ghcr.io/rochasamuel/havenkeys-server` by
pushing a tag named `server-vX.Y.Z`, where the version looks like `0.17.0`:

```sh
git tag server-v0.17.0 && git push origin server-v0.17.0
```

The first time, make the package public so others can pull it: GitHub, your
profile, Packages, `havenkeys-server`, Package settings, Change visibility,
Public.

Contributors can check the bundle end to end with
`deploy/compose/test/smoke.sh`.

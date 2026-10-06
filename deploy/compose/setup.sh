#!/bin/sh
# One-time setup (safe to re-run): writes .env, starts HavenKeys, checks it.
set -eu
cd "$(dirname "$0")"

say() { printf '%s\n' "$*"; }
die() { printf 'setup: %s\n' "$*" >&2; exit 1; }

command -v docker >/dev/null 2>&1 || die "Docker is not installed. See https://docs.docker.com/engine/install/"
command -v curl >/dev/null 2>&1 || die "curl is not installed; install it and run this again."
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
	case "$email" in
	*[[:space:]\"\'\$\#\\]*) die "the email contains characters that are not allowed." ;;
	esac
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

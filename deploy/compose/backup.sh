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

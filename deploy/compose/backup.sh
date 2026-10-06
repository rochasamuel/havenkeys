#!/bin/sh
# Database backups for the HavenKeys Compose bundle.
#   havenkeys-backup now    one dump now
#   havenkeys-backup loop   one dump every day at 03:00 (container TZ)
# Dumps: /backups/havenkeys-YYYY-MM-DD.dump (pg_dump custom format).
set -eu
umask 077

dump() {
	out="/backups/havenkeys-$(date +%F).dump"
	tmp="$out.partial"
	rm -f /backups/havenkeys-*.dump.partial
	pg_dump --format=custom --file="$tmp" || { rm -f "$tmp"; echo "backup: pg_dump failed" >&2; return 1; }
	[ -s "$tmp" ] || { rm -f "$tmp"; echo "backup: dump is empty" >&2; return 1; }
	mv "$tmp" "$out" || { rm -f "$tmp"; echo "backup: could not write $out" >&2; return 1; }
	# Retention runs only after a good dump, so a failing database never
	# eats the backups that are still good.
	find /backups -name 'havenkeys-*.dump' -type f -mtime +"${BACKUP_KEEP_DAYS:-14}" -delete ||
		echo "backup: retention cleanup failed" >&2
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

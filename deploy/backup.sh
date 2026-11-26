#!/bin/sh
# Backs up the PingCraft server stack into one archive: the homeserver's database, every bridge database,
# the bridge configuration and registrations, the generated secrets and `.env`.
#
# The homeserver and the bridges are stopped while it runs, so the copy is consistent, and started again after.
# The archive holds every user's network logins and the keys that decrypt their messages: keep it as secret as
# the server itself (ADR 0004). See docs/backup.md.
#
# Usage: ./backup.sh [output directory]   (default: ./backups, git-ignored)
set -eu

cd "$(dirname "$0")"

fail() { printf 'backup: %s\n' "$1" >&2; exit 1; }

[ -f .env ] || fail "no .env here: nothing to back up"
[ -d data ] || fail "no data/ here: run ./setup.sh first"

out_dir="${1:-backups}"
umask 077
mkdir -p "$out_dir"

project=$(docker compose config | sed -n 's/^name: //p')
[ -n "$project" ] || fail "could not read the compose project name"
stamp=$(date -u +%Y%m%dT%H%M%SZ)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

others=$(docker compose config --services | grep -vx 'bridge-db' || true)
echo "backup: stopping the homeserver and bridges"
# shellcheck disable=SC2086
docker compose stop $others
restart() {
	echo "backup: starting the stack again"
	docker compose up -d >/dev/null
}
trap 'restart; rm -rf "$work"' EXIT

docker compose up -d --wait bridge-db >/dev/null

echo "backup: dumping the bridge databases"
mkdir "$work/bridge-db"
for db in $(docker compose exec -T bridge-db psql -U bridges -d postgres -Atc \
	"select datname from pg_database where not datistemplate and datname <> 'postgres'"); do
	docker compose exec -T bridge-db pg_dump -U bridges -Fc "$db" > "$work/bridge-db/$db.dump"
done

echo "backup: copying the homeserver database"
docker compose run --rm --no-deps -T -v "${project}_homeserver:/homeserver:ro" --entrypoint tar bridge-db \
	-czf - -C /homeserver . > "$work/homeserver.tar.gz"

cp .env "$work/env"
tar -cf "$work/data.tar" data

archive="$out_dir/pingcraft-backup-$stamp.tar"
tar -cf "$archive" -C "$work" env data.tar homeserver.tar.gz bridge-db
chmod 600 "$archive"
echo "backup: wrote $archive. It is a secret: it can read users' messages and act on their networks."

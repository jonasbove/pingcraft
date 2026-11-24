#!/bin/sh
# Restores a backup made by backup.sh into an empty stack. It never overwrites: it refuses when data/ or the
# homeserver's volume already exist. See docs/backup.md.
#
# Usage: ./restore.sh <archive>
set -eu

cd "$(dirname "$0")"

fail() { printf 'restore: %s\n' "$1" >&2; exit 1; }

[ $# -eq 1 ] || fail "usage: ./restore.sh <archive>"
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
[ -f "$archive" ] || fail "no such archive: $1"
[ ! -e data ] || fail "data/ exists: restore only into an empty stack (move data/ away first)"

umask 077
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -xf "$archive" -C "$work"
for f in env data.tar homeserver.tar.gz bridge-db; do
	[ -e "$work/$f" ] || fail "archive is missing $f: not a PingCraft backup"
done

if [ -f .env ]; then
	cmp -s .env "$work/env" || fail ".env differs from the backup's: move it away, or restore it by hand"
else
	cp "$work/env" .env
fi

project=$(docker compose config | sed -n 's/^name: //p')
[ -n "$project" ] || fail "could not read the compose project name"
if docker volume inspect "${project}_homeserver" >/dev/null 2>&1; then
	fail "volume ${project}_homeserver exists: restore only into an empty stack"
fi

tar -xf "$work/data.tar"

echo "restore: creating the stack without starting it"
docker compose up --no-start >/dev/null

echo "restore: loading the homeserver database"
docker compose run --rm --no-deps -T -v "${project}_homeserver:/homeserver" --entrypoint tar bridge-db \
	-xzf - -C /homeserver < "$work/homeserver.tar.gz"

echo "restore: loading the bridge databases"
docker compose up -d --wait bridge-db >/dev/null
for dump in "$work"/bridge-db/*.dump; do
	db=$(basename "$dump" .dump)
	exists=$(docker compose exec -T bridge-db psql -U bridges -d postgres -Atc \
		"select 1 from pg_database where datname = '$db'")
	[ "$exists" = 1 ] || docker compose exec -T bridge-db createdb -U bridges "$db"
	docker compose exec -T bridge-db pg_restore -U bridges --clean --if-exists --no-owner -d "$db" < "$dump"
done

echo "restore: starting the stack"
docker compose up -d >/dev/null
echo "restore: done"

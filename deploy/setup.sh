#!/bin/sh
# One-time setup for the PingCraft server stack. Safe to run again: it never overwrites existing secrets or
# bridge configuration.
#
# Generates the secrets into git-ignored files (ADR 0004), creates the Telegram bridge's configuration from the
# bridge's own defaults plus the overrides below, and writes the registration files the homeserver loads.
# It never prints a secret.
set -eu

cd "$(dirname "$0")"

fail() { printf 'setup: %s\n' "$1" >&2; exit 1; }

[ -f .env ] || { cp .env.example .env; chmod 600 .env; fail "created .env: fill it in, then run ./setup.sh again"; }
# shellcheck disable=SC1091
. ./.env

[ -n "${PINGCRAFT_SERVER_NAME:-}" ] || fail "PINGCRAFT_SERVER_NAME is empty in .env. It is permanent: choose it with care."
[ -n "${TELEGRAM_API_ID:-}" ] || fail "TELEGRAM_API_ID is empty in .env"
[ -n "${TELEGRAM_API_HASH:-}" ] || fail "TELEGRAM_API_HASH is empty in .env"
ADMIN_LOCALPART="${PINGCRAFT_ADMIN_LOCALPART:-admin}"

umask 077
mkdir -p data/secrets data/appservices data/telegram

secret() { head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 48; }
new_secret_file() { [ -s "$1" ] || secret > "$1"; }

new_secret_file data/secrets/registration_token
new_secret_file data/secrets/bridge_db_password
new_secret_file data/secrets/doublepuppet_as_token
new_secret_file data/secrets/doublepuppet_hs_token

# Runs the Telegram bridge image as the current user, with the given entrypoint, without starting anything else.
tg() {
	entry="$1"; shift
	docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
		-e SN="$PINGCRAFT_SERVER_NAME" -e ADMIN="@$ADMIN_LOCALPART:$PINGCRAFT_SERVER_NAME" \
		-e TG_API_ID="$TELEGRAM_API_ID" -e TG_API_HASH="$TELEGRAM_API_HASH" \
		-e DB_PW="$(cat data/secrets/bridge_db_password)" \
		-e DP="$(cat data/secrets/doublepuppet_as_token)" \
		--entrypoint "$entry" telegram "$@"
}

if [ ! -s data/telegram/config.yaml ]; then
	echo "setup: writing the Telegram bridge configuration"
	tg /usr/bin/mautrix-telegram -c /data/config.yaml -e >/dev/null
	# PingCraft's overrides of the bridge's defaults. ADR 0004: encryption required, keys pickled, quiet logs.
	# Privacy: no portals are created automatically, no chats are synced at login, and history is not imported;
	# a chat is bridged only when the user starts it from Matrix.
	tg yq -i '
		.homeserver.address = "http://homeserver:8008" |
		.homeserver.domain = strenv(SN) |
		.appservice.hostname = "0.0.0.0" |
		.appservice.address = "http://telegram:" + (.appservice.port | tostring) |
		.database.type = "postgres" |
		.database.uri = "postgres://bridges:" + strenv(DB_PW) + "@bridge-db/telegram?sslmode=disable" |
		.network.api_id = env(TG_API_ID) |
		.network.api_hash = strenv(TG_API_HASH) |
		.network.sync.create_limit = 0 |
		.network.sync.login_sync_limit = 0 |
		.network.sync.direct_chats = false |
		.bridge.portal_create_filter.mode = "allow" |
		.bridge.portal_create_filter.list = [] |
		.bridge.permissions = {"*": "relay"} |
		.bridge.permissions[strenv(SN)] = "user" |
		.bridge.permissions[strenv(ADMIN)] = "admin" |
		.matrix.federate_rooms = false |
		.backfill.enabled = false |
		.encryption.allow = true |
		.encryption.default = true |
		.encryption.require = true |
		.encryption.pickle_key = "generate" |
		.double_puppet.secrets = {} |
		.double_puppet.secrets[strenv(SN)] = "as_token:" + strenv(DP) |
		.provisioning.shared_secret = "generate" |
		.logging = {"min_level": "warn", "writers": [{"type": "stdout", "format": "pretty"}]}
	' /data/config.yaml
fi

if [ ! -s data/telegram/registration.yaml ]; then
	echo "setup: generating the Telegram bridge registration"
	tg /usr/bin/mautrix-telegram -g -c /data/config.yaml -r /data/registration.yaml >/dev/null
fi
cp data/telegram/registration.yaml data/appservices/telegram.yaml

# One registration lets every bridge act as each local user's own Matrix account (double puppeting).
server_re=$(printf '%s' "$PINGCRAFT_SERVER_NAME" | sed 's/[.]/\\./g')
cat > data/appservices/doublepuppet.yaml <<EOF
id: doublepuppet
url: null
as_token: $(cat data/secrets/doublepuppet_as_token)
hs_token: $(cat data/secrets/doublepuppet_hs_token)
sender_localpart: _doublepuppet_$(secret | head -c 12)
rate_limited: false
namespaces:
  users:
    - regex: '@.*:${server_re}'
      exclusive: false
EOF

echo "setup: done. Start with: docker compose up -d"
echo "setup: create the first user with the registration token in data/secrets/registration_token"

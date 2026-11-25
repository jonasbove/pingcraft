#!/bin/sh
# One-time setup for the PingCraft server stack. Safe to run again: it never overwrites existing secrets or
# bridge configuration.
#
# Generates the secrets into git-ignored files (ADR 0004), creates each bridge's configuration from the
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
mkdir -p data/secrets data/appservices

secret() { head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 48; }
new_secret_file() { [ -s "$1" ] || secret > "$1"; }

new_secret_file data/secrets/registration_token
new_secret_file data/secrets/bridge_db_password
new_secret_file data/secrets/doublepuppet_as_token
new_secret_file data/secrets/doublepuppet_hs_token

# The stack's own certificate for the server name (ADR 0003 addendum): the proxy shows it to the bridges when they
# check a user's OpenID token, and only the bridges trust it. Self-signed, P-256, valid for 100 years: it never
# leaves the stack. The key goes to the proxy only.
case "$PINGCRAFT_SERVER_NAME" in
	*:*|localhost|*[!A-Za-z0-9.-]*) fail "PINGCRAFT_SERVER_NAME must be a plain host name, such as example.org: the bridges cannot check sign-ins for '$PINGCRAFT_SERVER_NAME'" ;;
esac
if [ ! -s data/openid-tls/key.pem ]; then
	command -v openssl >/dev/null 2>&1 || fail "openssl is needed once, to make the stack's internal certificate"
	echo "setup: making the stack's internal certificate for $PINGCRAFT_SERVER_NAME"
	mkdir -p data/openid-tls data/openid-ca
	openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 36500 \
		-subj "/CN=$PINGCRAFT_SERVER_NAME" -addext "subjectAltName=DNS:$PINGCRAFT_SERVER_NAME" \
		-addext "basicConstraints=critical,CA:FALSE" -addext "extendedKeyUsage=serverAuth" \
		-keyout data/openid-tls/key.pem -out data/openid-tls/cert.pem 2>/dev/null
	cp data/openid-tls/cert.pem data/openid-ca/pingcraft-openid.crt
fi

# Runs a bridge image as the current user, with the given entrypoint, without starting anything else.
#   run_bridge <service> <entrypoint> [args...]
run_bridge() {
	service="$1"; entry="$2"; shift 2
	docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
		-e SN="$PINGCRAFT_SERVER_NAME" -e ADMIN="@$ADMIN_LOCALPART:$PINGCRAFT_SERVER_NAME" \
		-e TG_API_ID="$TELEGRAM_API_ID" -e TG_API_HASH="$TELEGRAM_API_HASH" \
		-e DB_PW="$(cat data/secrets/bridge_db_password)" \
		-e DP="$(cat data/secrets/doublepuppet_as_token)" \
		--entrypoint "$entry" "$service" "$@"
}

# PingCraft's overrides of every bridge's defaults (all bridges are mautrix bridgev2), with the bridge's service
# name, which is also its Postgres database. ADR 0004: encryption required, keys pickled, quiet logs.
# Chats (maintainer, 2026-11-25): a chat is created when a message arrives in it, or when the user starts it, with
# messages from then on. No chats are made at login and no history is imported.
common_overrides() {
	cat <<YQ
		.homeserver.address = "http://proxy:8008" |
		.homeserver.domain = strenv(SN) |
		.appservice.hostname = "0.0.0.0" |
		.appservice.address = "http://$1:" + (.appservice.port | tostring) |
		.database.type = "postgres" |
		.database.uri = "postgres://bridges:" + strenv(DB_PW) + "@bridge-db/$1?sslmode=disable" |
		.bridge.portal_create_filter.mode = "deny" |
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
YQ
}

# Writes a bridge's configuration (defaults, then the common overrides, then the network's own) and its
# registration, once, and hands the registration to the homeserver.
#   setup_bridge <service> <binary> <network overrides>
setup_bridge() {
	service="$1"; binary="$2"; network="$3"
	mkdir -p "data/$service"
	if [ ! -s "data/$service/config.yaml" ]; then
		echo "setup: writing the $service bridge configuration"
		run_bridge "$service" "$binary" -c /data/config.yaml -e >/dev/null
		run_bridge "$service" yq -i "$(common_overrides "$service") | $network" /data/config.yaml
	fi
	# The bridges reach the homeserver through the proxy, like every client: the homeserver takes each client's
	# address from the X-Real-IP header the proxy sets (Caddyfile), and fails requests that come without it.
	# Configurations written before the proxy existed are moved over once.
	if grep -q 'address: http://homeserver:8008$' "data/$service/config.yaml"; then
		echo "setup: pointing the $service bridge at the proxy"
		run_bridge "$service" yq -i '.homeserver.address = "http://proxy:8008"' /data/config.yaml
	fi
	# Configurations written before 2026-11-25 allowed no chats at all, so incoming messages were dropped. Move them
	# to "a chat for each new message" once; a filter someone has edited (a non-empty list) is left alone.
	if grep -A6 '^    portal_create_filter:' "data/$service/config.yaml" | grep -q '^        mode: allow$' &&
		grep -A10 '^    portal_create_filter:' "data/$service/config.yaml" | grep -q '^        list: \[\]$'; then
		echo "setup: letting the $service bridge create a chat when a message arrives"
		run_bridge "$service" yq -i '.bridge.portal_create_filter.mode = "deny"' /data/config.yaml
	fi
	if [ ! -s "data/$service/registration.yaml" ]; then
		echo "setup: generating the $service bridge registration"
		run_bridge "$service" "$binary" -g -c /data/config.yaml -r /data/registration.yaml >/dev/null
	fi
	cp "data/$service/registration.yaml" "data/appservices/$service.yaml"
}

# Telegram (ADR 0005): no chats are synced at login.
setup_bridge telegram /usr/bin/mautrix-telegram '
		.network.api_id = env(TG_API_ID) |
		.network.api_hash = strenv(TG_API_HASH) |
		.network.sync.create_limit = 0 |
		.network.sync.login_sync_limit = 0 |
		.network.sync.direct_chats = false'

# Signal (ADR 0006): the linked device is named PingCraft on the phone, and nothing is fetched from other servers
# (the upstream note-to-self avatar lives on a federated server, and locations would link to an online map).
setup_bridge signal /usr/bin/mautrix-signal '
		.network.device_name = "PingCraft" |
		.network.note_to_self_avatar = "" |
		.network.location_format = "geo:%[1]s,%[2]s"'

# WhatsApp (ADR 0007): the linked device is named PingCraft on the phone, and status updates are not bridged.
setup_bridge whatsapp /usr/bin/mautrix-whatsapp '
		.network.os_name = "PingCraft" |
		.network.enable_status_broadcast = false'

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

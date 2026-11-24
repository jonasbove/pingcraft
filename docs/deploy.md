# Running the PingCraft server

The server side is one Docker Compose project in [`deploy/`](../deploy/):

- **a reverse proxy (Caddy):** the one way in, for the homeserver and for connecting networks from PingCraft;
- **tuwunel:** the Matrix homeserver (ADR 0001);
- **one container per bridge:** today, Telegram, Signal and WhatsApp;
- **Postgres:** the bridges' database.

Everything below runs from the `deploy/` folder. You need Docker with Compose v2.

## What the server holds

Read this before you start. A bridge has to read your messages to carry them between a network and Matrix
(ADR 0004), so the server's data includes:

- every connected account's login to its network;
- the bridges' encryption keys, which can decrypt the bridged chats;
- the messages of any unencrypted room.

Whoever can read the server's volumes, its `data/` folder or a backup can read your chats and act on your
networks. Run it on a machine you trust, and treat backups the same way ([backup.md](backup.md)).

## 1. Choose the server name

Every user ID ends in the server name (`@you:example.com`). **It can never change** without starting over with an
empty server. Use a domain you own, as a plain host name: no port, and not `localhost` or an IP address, or the
bridges cannot check who is connecting a network (see "How PingCraft reaches the bridges" below). If the homeserver will live on a subdomain (`matrix.example.com`) but you want
IDs on the bare domain, set up `.well-known` delegation on the bare domain (see step 6).

## 2. Fill in `.env`

```sh
cp .env.example .env
chmod 600 .env
```

| Setting | What it is |
|---|---|
| `PINGCRAFT_SERVER_NAME` | the permanent server name from step 1. Required |
| `PINGCRAFT_ADMIN_LOCALPART` | the first user's name, who gets admin rights on the bridges (default `admin`) |
| `PINGCRAFT_BIND`, `PINGCRAFT_PORT` | where the stack listens on the host (default `127.0.0.1:8008`) |
| `PINGCRAFT_OPENID_SUBNET` | optional: the internal network the bridges use to check sign-ins (default `198.18.0.0/24`). Change it only if the host already uses that range |
| `PINGCRAFT_UID`, `PINGCRAFT_GID` | the host user that owns `data/`, from `id -u` and `id -g` |
| `TELEGRAM_API_ID`, `TELEGRAM_API_HASH` | from an application you register at <https://my.telegram.org> under "API development tools". Secret (ADR 0005) |

`.env` is git-ignored. Never commit it or paste it anywhere.

## 3. Run setup

```sh
./setup.sh
```

It stops with an error if the server name or the Telegram values are missing. Otherwise it:

- generates the secrets into `data/secrets/`;
- makes the stack's own certificate for the server name in `data/openid-tls/` (with its private key) and
  `data/openid-ca/` (the certificate alone, for the bridges); this needs `openssl` once;
- writes each bridge's configuration to `data/<bridge>/` (`data/telegram/`, `data/signal/`, `data/whatsapp/`),
  starting from the bridge's own defaults;
- writes the registrations tuwunel loads from `data/appservices/`.

It never prints a secret, and it never overwrites anything that exists, so it is safe to run again.

The bridge configuration it writes:

- encryption on the Matrix side is required;
- no chat history is imported;
- no chat is bridged automatically: you choose each one;
- logs show warnings and errors only.

## 4. Start it

```sh
docker compose up -d
```

On a copy-on-write filesystem such as btrfs, tuwunel logs a warning about RocksDB preallocating its write-ahead
log. You can turn that preallocation off in `tuwunel.toml` (see `rocksdb_allow_fallocate` in tuwunel's example
configuration).

## 5. Create users

Registration is closed to everyone without the registration token in `data/secrets/registration_token`. To add a
user, open a Matrix client such as Element, choose "Create account" on your homeserver, and enter the token when
asked.

The first user created becomes the homeserver's admin. Create the admin first, with the name in
`PINGCRAFT_ADMIN_LOCALPART`. Give the token only to people you are adding.

## 6. Reach it from other devices: HTTPS

Matrix clients refuse a homeserver on plain `http://` unless it is `localhost`. To use the server from anywhere
else:

- put a reverse proxy with HTTPS in front of it (Caddy, nginx, Traefik, ...), forwarding to `PINGCRAFT_PORT`;
- leave `PINGCRAFT_BIND` on `127.0.0.1` if the proxy runs on the same machine;
- for IDs on the bare domain while the server runs on a subdomain, serve `/.well-known/matrix/client` on the bare
  domain, pointing at the subdomain.

For a quick test from one other computer, an SSH tunnel to `localhost` works without HTTPS:

```sh
ssh -N -L 8008:127.0.0.1:8008 <server>
```

The client then uses `http://localhost:8008`. Running next to CraftCloud is covered in milestone 4.

## 7. How PingCraft reaches the bridges

PingCraft connects a network through that network's bridge, with the bridge's provisioning API (ADR 0003). The
proxy serves it next to the homeserver, one path per bridge:

| Path on the stack's address | Goes to |
|---|---|
| `/_pingcraft/bridges/telegram/_matrix/provision/v3/…` | the Telegram bridge |
| `/_pingcraft/bridges/signal/_matrix/provision/v3/…` | the Signal bridge |
| `/_pingcraft/bridges/whatsapp/_matrix/provision/v3/…` | the WhatsApp bridge |
| everything else | the homeserver |

Only the provisioning API is passed on, never a bridge's other endpoints. An HTTPS reverse proxy in front (step 6)
needs nothing extra: it forwards everything to `PINGCRAFT_PORT`, these paths included.

PingCraft never gives a bridge your password or your access token. It asks the homeserver for an OpenID token,
which only proves who you are, and the bridge checks it with the homeserver. A bridge does that check the way
servers check each other, over HTTPS at the server name, and refuses private addresses. Federation stays off, so
the stack answers that one check itself:

- the proxy also sits on a second internal network, `openid` (`PINGCRAFT_OPENID_SUBNET`), where the server name
  points at it;
- there it shows the certificate from `data/openid-tls/`, which the bridges trust through `data/openid-ca/`;
- it forwards only the OpenID check, `/_matrix/federation/v1/openid/userinfo`, to the homeserver.

Nothing outside the stack reaches that network. ADR 0003's addendum says why it is built this way.

**A stack set up before the proxy and WhatsApp were added** needs these steps once:

1. Create the WhatsApp bridge's database with `docker compose exec bridge-db createdb -U bridges whatsapp`.
2. Run `./setup.sh`. It makes the certificate and the WhatsApp bridge's configuration, and keeps everything that
   exists.
3. Run `docker compose up -d`. The proxy now takes over `PINGCRAFT_PORT` from the homeserver, so the address
   clients use does not change.

## 8. Connect Telegram

Until PingCraft's own client can do this (milestone 3), use any Matrix client.

1. **Open the bridge's room:** start a direct message with `@telegrambot:<server name>`, and check that the room
   is encrypted. The bridge ignores unencrypted messages.
2. **Log in:** send `login phone` (or `login qr`), then answer each question in its own message: your number with
   its country code, the code Telegram sends to your app, then your two-step password if you have one.
3. **Bridge one chat:** send `start-chat <Telegram username>`. Nothing is bridged until you do this for a chat.

Read ADR 0005 for what Telegram allows and what can get an account restricted.

## 9. Connect Signal

Signal has no separate login: the bridge becomes a linked device of the Signal app on your phone, like Signal's
own desktop app. Read ADR 0006 first.

1. **Open the bridge's room:** start a direct message with `@signalbot:<server name>`, and check that the room is
   encrypted.
2. **Log in:** send `login`. The bridge answers with a QR code. On your phone, open Signal, then Settings, Linked
   devices, Link new device, and scan it. The device shows up on the phone as "PingCraft".
3. **Chats:** a chat appears in Matrix when a message arrives in it. To message someone first, send
   `start-chat <phone number with country code>`.

To disconnect, send `logout` in the bridge's room, or remove "PingCraft" under Linked devices on the phone.

A stack created before Signal was added has no `signal` database. Create it once with
`docker compose exec bridge-db createdb -U bridges signal`, then run `./setup.sh` and `docker compose up -d`.

## 10. Connect WhatsApp

Like Signal, the bridge becomes a linked device of WhatsApp on your phone. Read ADR 0007 first: WhatsApp's terms
do not allow unofficial apps, and it bans accounts it finds suspicious. Your phone must come online at least every
14 days, or the link drops.

1. **Open the bridge's room:** start a direct message with `@whatsappbot:<server name>`, and check that the room
   is encrypted.
2. **Log in:** send `login qr`. The bridge answers with a QR code. On your phone, open WhatsApp, then Linked
   devices, Link a device, and scan it. The device shows up on the phone as "PingCraft". `login phone` gives an
   eight-character code to type on the phone instead.
3. **Chats:** a chat appears in Matrix when a message arrives in it.

To disconnect, send `logout` in the bridge's room, or remove "PingCraft" under Linked devices on the phone.

## Updates

Every image in `compose.yaml` is pinned by version and digest. Updates arrive as pull requests that move those
pins; nothing updates itself (ADR 0010). To update:

1. Take a backup ([backup.md](backup.md)).
2. Pull the new version of this repository.
3. Run `docker compose up -d`.

A homeserver upgrade can run a database migration on its first start. Compose waits up to 30 minutes for it
before stopping the container, so do not interrupt it.

## Removing it

**Log every account out of its network first.** Send `logout` in each bridge's room, so the network ends the
session too. Then:

```sh
docker compose down -v
```

`-v` deletes the volumes: the homeserver's and the bridges' databases. Delete `data/` (and `.env` when you no
longer need the API values) yourself.

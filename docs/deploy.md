# Running the PingCraft server

The server side is one Docker Compose project in [`deploy/`](../deploy/):

- **tuwunel:** the Matrix homeserver (ADR 0001);
- **one container per bridge:** today, Telegram and Signal;
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
empty server. Use a domain you own. If the homeserver will live on a subdomain (`matrix.example.com`) but you want
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
| `PINGCRAFT_BIND`, `PINGCRAFT_PORT` | where the homeserver listens on the host (default `127.0.0.1:8008`) |
| `PINGCRAFT_UID`, `PINGCRAFT_GID` | the host user that owns `data/`, from `id -u` and `id -g` |
| `TELEGRAM_API_ID`, `TELEGRAM_API_HASH` | from an application you register at <https://my.telegram.org> under "API development tools". Secret (ADR 0005) |

`.env` is git-ignored. Never commit it or paste it anywhere.

## 3. Run setup

```sh
./setup.sh
```

It stops with an error if the server name or the Telegram values are missing. Otherwise it:

- generates the secrets into `data/secrets/`;
- writes each bridge's configuration to `data/<bridge>/` (`data/telegram/`, `data/signal/`), starting from the
  bridge's own defaults;
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

## 7. Connect Telegram

Until PingCraft's own client can do this (milestone 3), use any Matrix client.

1. **Open the bridge's room:** start a direct message with `@telegrambot:<server name>`, and check that the room
   is encrypted. The bridge ignores unencrypted messages.
2. **Log in:** send `login phone` (or `login qr`), then answer each question in its own message: your number with
   its country code, the code Telegram sends to your app, then your two-step password if you have one.
3. **Bridge one chat:** send `start-chat <Telegram username>`. Nothing is bridged until you do this for a chat.

Read ADR 0005 for what Telegram allows and what can get an account restricted.

## 8. Connect Signal

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

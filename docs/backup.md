# Backing up and restoring the PingCraft server

## What a backup contains, and why it is a secret

`deploy/backup.sh` writes one archive with everything the server needs to come back:

- the homeserver's database (users, rooms, and the stored messages, most of them encrypted);
- every bridge's database, including the logins to each network and the bridges' encryption keys;
- `data/`, which holds the bridge configuration, the registrations, the generated secrets and the stack's internal
  certificate with its private key;
- `.env`, which holds the server name and network API values.

**Anyone holding the archive can read the bridged chats and act on the connected accounts** (ADR 0004). The
script makes the file readable by its owner only. Keep it that way:

- store it encrypted, for example in an encrypted backup tool or on an encrypted disk;
- never put it anywhere shared or public.

## Back up

From `deploy/`:

```sh
./backup.sh              # writes backups/pingcraft-backup-<UTC time>.tar (backups/ is git-ignored)
./backup.sh /some/where  # or into another folder
```

While it runs, it stops the homeserver and the bridges, so nothing changes mid-copy. It dumps each bridge
database, copies the homeserver's database, and then starts everything again. Messages sent to you meanwhile wait
on their networks and arrive once the bridges are back.

Take a backup before every update (see [deploy.md](deploy.md)).

## Restore

`deploy/restore.sh` restores into an **empty** stack only. It refuses to run while `data/` or the homeserver's
volume exist, so it can never overwrite a running server. To restore on a new machine, or after removing a
broken stack:

```sh
# in deploy/, with no data/ folder and no volumes left (docker compose down -v)
./restore.sh /path/to/pingcraft-backup-<time>.tar
```

It puts back `.env`, unless one is already there, in which case the two must match. Then it puts back `data/`,
loads the homeserver's database into a new volume, restores each bridge database, and starts the stack.

## What was tested

On 2026-11-24, with throwaway data and a fake Telegram application:

1. A stack was seeded with a user, a room and a message.
2. It was backed up, then deleted with `docker compose down -v` and `rm -rf data`.
3. `restore.sh` refused to run while `data/` existed.
4. The backup was restored with `restore.sh`. Afterwards:
   - the user could log in;
   - the message was in its room;
   - the bridge database had all its tables;
   - the bridge bot still had its encryption device.

Restoring with a connected network account was not tested.

## Limits

- Only the state at backup time comes back. Matrix messages sent after the backup are lost; messages waiting on
  a network may arrive again once the bridge reconnects.
- A restored bridge reconnects with the same network session. Do not run the old and the restored stack at the
  same time: two bridges using one session can get the account logged out or flagged.
- tuwunel also has its own online database backup option (`[global.database.backup]`). It is not used here,
  because it covers the homeserver only and has not been tested with PingCraft.

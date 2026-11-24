# ADR 0001: tuwunel is the homeserver

- **Status:** accepted by the maintainer (2026-11-24), subject to the milestone 1 spike below

## Context

PingCraft needs a Rust, Apache-2.0 Matrix homeserver that works with appservice bridges and with
`matrix-sdk-ui`, whose room list and sync service need Simplified Sliding Sync (MSC4186). The
candidates in November 2026 were continuwuity (v26.9.4) and tuwunel (v2.0.0, 2026-11-10). Both are
Apache-2.0, RocksDB-based and support MSC4186. conduwuit, which both forked from, is archived.
Their databases cannot be migrated into each other, so the choice is effectively one-way.

## Decision

Use tuwunel. It reads bridge registrations from a directory of YAML files (`appservice_dir`), so
one compose file can register bridges declaratively. It supports MSC4190 (bridges manage their own
devices), the MSC3202 transaction extensions and to-device messages for appservices, and its 2.0
release fixed issues specifically against `matrix-sdk` 0.19.1.

Milestone 1 opens with a spike that runs one bridge with end-to-bridge encryption against tuwunel
from a stock Matrix client. If that fails in a way upstream cannot fix, this decision is revisited
before anything builds on it.

## Alternatives

- **continuwuity:** the original candidate. Bridges can only be registered with an admin-room
  command, and it does not forward to-device messages to appservices, so bridges must use
  sync-based encryption. It is the recorded fallback.
- **Conduit:** maintained, but MSC4190 support was not found in its source. MSC4190 is needed for encrypted
  bridges when the homeserver uses OAuth-based login.
- **Synapse:** the reference server and the only one mautrix tests against, but AGPL-3.0, Python, and
  Postgres for production use. Memory use on a small machine was not measured for any candidate.
- **Dendrite:** security fixes only.

## Consequences

- Switching homeserver later means starting from an empty database: chats re-sync from the
  networks, and history that only lived on the server is lost.
- mautrix documents only Synapse, so bridge problems on tuwunel are ours to diagnose first.
- tuwunel's 2.0 brought build and config changes; pin a version and read release notes on upgrade.

## Addendum (2026-11-24): the milestone 1 spike

tuwunel v2.0.0 and mautrix-telegram v0.2611.0, from `deploy/compose.yaml`, with a stock Matrix client (Element
Desktop) over a private network. The decision stands.

- **Registration:** tuwunel loaded the bridge's and the double-puppet registrations from `appservice_dir`, with
  no admin-room command.
- **Encryption:** the bridge bot's device got its identity keys, and the bridged room was encrypted
  (`m.megolm.v1.aes-sha2`). Every message stored on the homeserver for that room was `m.room.encrypted`.
- **Both directions:** a message sent from Matrix arrived on Telegram, and one sent from Telegram arrived on
  Matrix.
- **Only what the user asks for is bridged:** with automatic portal creation filtered, chat sync limits at zero
  and backfill off, the login created no Matrix rooms. One chat was bridged with `start-chat`. The bridge still
  keeps database rows for a few recent chats.
- **Bridge-info state:** the bridged room carries both `m.bridge` and `uk.half-shot.bridge` with
  `protocol.id = "telegram"`, under a **non-empty state key**. The client must read every state key of those
  types (ADR 0003). The bridge also creates a space per login, which carries `m.bridge` with an empty state key.
- **OpenID with federation off:** tuwunel answered `/_matrix/federation/v1/openid/userinfo` with the right user
  while federation was disabled. Whether a bridge can reach it inside compose under the server name is still the
  milestone 3 spike.
- **Memory** (`docker stats`, idle → one logged-in account with one bridged chat):

  | Container | Idle | One account, one chat |
  |---|---|---|
  | tuwunel | 71 MiB | 81 MiB |
  | Postgres | 32 MiB | 36 MiB |
  | mautrix-telegram | 9 MiB | 12 MiB |

  These are small-scale figures, not a load test.

Findings for later tickets:

- **HTTPS is needed:** browsers, and Element Desktop, refuse a plain-`http://` homeserver at any address other
  than `localhost` (mixed content). The deployment docs must require HTTPS (milestone 4), and a test from
  another machine needs a tunnel to `localhost`.
- **The bridge bot shows as unverified:** with `encryption.self_sign` off, clients flag its messages. Decide
  whether to enable it; the bridge docs tie it to MSC4190.
- **Bridge logs are sensitive:** even warning-level lines include network user IDs (ADR 0004).
- **Copy-on-write filesystems:** on one, tuwunel warns that RocksDB's write-ahead log preallocation misbehaves,
  and suggests turning `rocksdb_allow_fallocate` off. Document it for btrfs hosts.

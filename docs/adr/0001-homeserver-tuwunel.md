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
- **Conduit:** maintained but without MSC4190, which encrypted mautrix bridges need.
- **Synapse:** the reference server and the only one mautrix tests against, but AGPL-3.0 and
  Python, with Postgres; heavier on a small machine.
- **Dendrite:** security fixes only.

## Consequences

- Switching homeserver later means starting from an empty database: chats re-sync from the
  networks, and history that only lived on the server is lost.
- mautrix documents only Synapse, so bridge problems on tuwunel are ours to diagnose first.
- tuwunel's 2.0 brought build and config changes; pin a version and read release notes on upgrade.

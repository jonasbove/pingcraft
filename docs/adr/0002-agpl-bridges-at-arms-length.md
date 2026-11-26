# ADR 0002: The AGPL bridges run unmodified, at arm's length

- **Status:** accepted by the maintainer (2026-11-24), as the licensing decision house rule 9 reserves for them

## Context

The four first networks are bridged by mautrix bridges (mautrix-whatsapp, mautrix-signal, mautrix-telegram,
mautrix-discord), all AGPL-3.0 and written in Go. mautrix-signal also compiles in Signal's `libsignal`, which is
AGPL-3.0. AGPL-3.0 is not on the Craft licence allowlist, which governs PingCraft's own dependencies and assets.

AGPL obligations attach to modifying a program and to conveying or offering a modified version. Running an
unmodified program as a separate process places no obligation on the programs that talk to it over a network
protocol. This is the project's reading, not legal advice.

Avoiding AGPL entirely is not possible for Signal: the only usable implementation of its protocol is `libsignal`,
and the Rust client library built on it (`presage`) is AGPL-3.0 as well. For WhatsApp and Telegram, permissive
protocol libraries exist (`whatsmeow`, MPL-2.0; `gotd`, MIT; `grammers`, Apache-2.0), as does the bridge framework
`mautrix-go` (MPL-2.0), but writing and maintaining our own bridges would be a large, permanent cost: these
networks change their protocols often and the upstream bridges ship monthly releases to keep up.

## Decision

1. PingCraft runs each bridge as the official upstream container image, unmodified, in its own container.
2. PingCraft talks to a bridge only over Matrix: the appservice protocol between bridge and homeserver, and the
   bridge's provisioning API over HTTP (ADR 0003). No bridge code is linked into, copied into or vendored in
   PingCraft.
3. We do not build, patch or redistribute bridge images. The compose files reference upstream images by version
   and digest, and a `NOTICE` names each bridge, its licence and where its source is.
4. A change a bridge needs goes upstream. Running a modified bridge needs a new ADR.

## Alternatives

- **Our own bridges on permissive libraries:** possible for WhatsApp and Telegram, impossible for Signal, and a
  maintenance burden larger than the client itself. Kept as the exit below.
- **Build and publish our own images from unmodified source:** easier to pin and audit, but then we convey AGPL
  software and owe a source offer. Not needed while upstream publishes images.
- **Leave out the AGPL bridges:** leaves PingCraft without its first networks.

## Consequences

- PingCraft's own code and dependencies stay on the allowlist; `cargo deny` never sees a bridge.
- Bridge upgrades follow upstream's releases; a regression is fixed upstream or by pinning the previous digest.
- **Exit:** if the maintainer rejects this rule, Telegram and WhatsApp can be re-bridged on permissive libraries
  behind the same bridge provider interface (ADR 0003), and Signal is dropped from the first networks.

# ADR 0006: Signal, risk level medium

- **Status:** accepted by the maintainer (2026-11-24)

## Context

Signal is bridged by mautrix-signal (AGPL-3.0, Go, bridgev2; v0.2611.0 on 2026-11-16), which compiles in Signal's
`libsignal` (AGPL-3.0, Rust) and runs it through FFI. It logs in as a linked device: the user scans a QR code
with the Signal app on their phone. Registering the bridge as the primary device is not supported.

Signal's terms (https://signal.org/legal/) forbid using the service in unauthorised ways and creating accounts
by automated means; they do not name third-party clients. A linked device uses the same mechanism as Signal's
own desktop app.

## Decision

- Risk level: **medium**: the terms are vague about third-party clients, and Signal may change the protocol
  in ways that break the bridge until upstream catches up.
- PingCraft shows the risk and the user confirms before connecting a Signal account.

## Alternatives

- None without AGPL: every usable Signal implementation builds on `libsignal` (ADR 0002).

## Consequences

- If the maintainer ever rejects ADR 0002, Signal is dropped rather than re-bridged.
- The bridge's database holds the linked device's Signal keys; anyone who can read it can read the account's
  messages.

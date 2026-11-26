# PingCraft

PingCraft puts all your chats in one inbox: WhatsApp, Signal, Telegram and Discord first, more later. It is a
self-hosted alternative to Beeper. You run it yourself, on your own computer or next to your
[CraftCloud](https://github.com/jonasbove/craftcloud) server, and your messages never pass through a service you
don't control.

PingCraft is a Craft app: native Rust and egui, following the house rules in [`AGENTS.md`](AGENTS.md).

## Decided

These were decided by the maintainer before work began. Changing one needs a new ADR in `docs/adr/` that the
maintainer approves.

- **Matrix underneath.** PingCraft is a Matrix client. Other networks are reached through Matrix bridges, the
  same design Beeper uses, so each network is a bridge you can add or leave out.
- **The server side** is a Rust Matrix homeserver (Apache-2.0; tuwunel, see [ADR 0001](docs/adr/0001-homeserver-tuwunel.md)) plus the
  mautrix bridges, each in its own container, behind one reverse proxy (Caddy; ADR 0003's addendum, approved
  2026-11-25).
- **The client** is Rust and egui on `matrix-rust-sdk` (Apache-2.0), desktop first, with a web build (Trunk)
  so CraftCloud can serve it like its other apps.
- **First networks:** WhatsApp, Signal, Telegram and Discord.
- **Two ways to run it:** locally (one compose file on your own computer), or alongside a CraftCloud server.
  How the CraftCloud option works is a Phase 0 question (see `PLAN.md`).

## Things to get right

- **People's messages come first.** Losing or leaking messages is the worst bug this project can have. End-to-end
  encryption keys, bridge logins and session tokens are secrets: never logged, never committed, never sent
  anywhere but the server they belong to.
- **Clean room.** Learn from Beeper's and Element's behaviour and from the Matrix specification. Do not copy their
  code, strings, icons or artwork. "WhatsApp", "Signal", "Telegram" and "Discord" appear only as the names of
  networks you connect, never as the name of our own feature.
- **Licences.** The client's dependencies follow the allowlist in `AGENTS.md`. The mautrix bridges are
  AGPL-3.0, which is not on the allowlist. They run as separate, unmodified programs in their own containers and
  are never linked into PingCraft, but whether that is acceptable is a maintainer decision: Phase 0 writes it up
  as an ADR and stops for approval (ADR 0002 records the approval).
- **Networks' terms.** Bridging some networks breaks their terms of service or can get an account banned.
  Each network's ADR says what the risk is, so people can choose.
- **Shared libraries first.** Check `storytold/craft-libs` (for example `settings`, `i18n`, `keymap`, `menu`,
  `dialogs`, `ui`, `platform`, `jsonrpc`, `cloud`) before writing anything reusable.

## Quality gates

Until the workspace exists, Phase 0 decides them. They will match the other Craft apps: `cargo xtask ci` running
fmt, clippy `-D warnings`, tests, layers and the wasm build, and egui_kittest tests for UI changes that check the
AccessKit tree.

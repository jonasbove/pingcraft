# PingCraft plan

## Phase 0: research and decisions (no app code)

Stop after this phase with a short report and wait for the maintainer's review.

1. **Survey.** What Beeper does and how (Matrix plus bridges, its "bridge manager", the self-hosted
   `bbctl` setup), and what open alternatives exist. Read the Matrix client-server spec parts PingCraft needs.
2. **Homeserver.** Compare continuwuity with the other Rust and non-Rust homeservers that support appservices
   (bridges) well: licence, maintenance, memory use on a small machine, database, federation needs (PingCraft can
   run without federation), and how bridges register.
3. **Bridges.** For WhatsApp, Signal, Telegram and Discord: which mautrix bridge (mautrix-whatsapp,
   mautrix-signal, mautrix-telegram, mautrix-discord), its licence, language, how you log in (QR code, phone
   number, token), end-to-end encryption support, how it stores secrets, how stable it is, and the account risk
   under the network's terms.
4. **Client.** `matrix-rust-sdk` and its UI crates (room list, timeline, encryption, sliding sync), what works
   on `wasm32-unknown-unknown`, and what `storytold/craft-libs` already provides. Sketch the crate layout with
   layering in the style of the other Craft apps (nothing below the UI crate depends on egui).
5. **Running it.** One compose file for local use; how it sits next to a CraftCloud server (separate compose
   project, CraftCloud's apps list serving the web client, shared sign-in or not); backups; updates.
6. **Write it down.**
   - ADRs in `docs/adr/` (`NNNN-title.md`: context, decision, alternatives, consequences), at least one per
     big choice: homeserver, bridge set and the AGPL question, client stack, encryption and key storage,
     deployment and the CraftCloud option, and one per network covering its account risk.
   - `docs/GLOSSARY.md`: the terms PingCraft uses (network, bridge, inbox, chat, account, ...), so the code,
     the UI and these docs use the same words.
   - Rewrite this file's later phases as concrete milestones, each one a set of tickets an agent session can
     finish on its own.
7. **Open questions** for the maintainer, as a list at the end of the report. Do not decide licensing,
   publishing, spending or anything hard to undo on your own.

## Later phases (to be rewritten in Phase 0)

1. Server stack running locally: homeserver plus one bridge, end to end, from a stock Matrix client.
2. PingCraft client: sign in, chat list, read and send text, end-to-end encryption.
3. All four first networks bridged; connecting a network from inside PingCraft.
4. Web build and the CraftCloud option.
5. Polish: notifications, search, media, a unified inbox across networks.

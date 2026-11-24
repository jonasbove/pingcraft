# PingCraft plan

Words follow [`GLOSSARY.md`](GLOSSARY.md). Decisions live in [`docs/adr/`](docs/adr/).

Each ticket below is sized for one agent session. Tickets marked **(maintainer)** need something only the
maintainer can give: a test account, an approval, or a change in another repository. Never use real accounts or
phone numbers; bridge tests use test accounts the maintainer provides.

## Phase 0: research and decisions (awaiting maintainer review)

Discussed with the maintainer on 2026-11-24:

| ADR | Decision |
|---|---|
| [0001](docs/adr/0001-homeserver-tuwunel.md) | tuwunel is the homeserver, confirmed by a spike in milestone 1 |
| [0002](docs/adr/0002-agpl-bridges-at-arms-length.md) | the AGPL bridges run unmodified at arm's length (**proposed** until formal approval) |
| [0003](docs/adr/0003-swappable-bridge-providers.md) | bridges sit behind a swappable bridge provider |
| [0004](docs/adr/0004-encryption-and-key-storage.md) | encryption, and where plaintext lives |
| [0005](docs/adr/0005-network-telegram.md) | Telegram, risk level low |
| [0006](docs/adr/0006-network-signal.md) | Signal, risk level medium |
| [0007](docs/adr/0007-network-whatsapp.md) | WhatsApp, risk level medium-high |
| [0008](docs/adr/0008-network-discord.md) | Discord comes last, with bot login by default |
| [0009](docs/adr/0009-client-stack-and-targets.md) | client crates, layers, build targets and quality gates |
| [0010](docs/adr/0010-deployment-and-craftcloud.md) | deployment, and running next to CraftCloud |

## Quality gates

These apply from milestone 2 on (ADR 0009):
- `cargo xtask ci` runs fmt, clippy `-D warnings`, tests, layers, the wasm check and `cargo deny`.
- Every crate root carries the never-crash `deny` attribute.
- UI changes get `egui_kittest` tests of the AccessKit tree.
- CI calls `storytold/craft-common-actions` and builds every target CraftCloud ships.

Milestone 1 has no Rust code. Its gate is the end-to-end check below, written up with what ran and what did not.

## Milestone 1: server stack, locally

Goal: tuwunel and mautrix-telegram, with encryption, in one compose project, checked end to end from a stock
Matrix client.

1. **Spike (maintainer: a Telegram test account and an `api_id`).** Run tuwunel and mautrix-telegram with
   Matrix-side encryption required. Log in to the bridge, then send and receive text both ways from a stock
   Matrix client. Record the findings as an addendum to ADR 0001, and stop if tuwunel cannot do it.
2. **`compose.yaml`** for the `pingcraft` project:
   - tuwunel, Postgres for the bridges, and the Telegram bridge;
   - images pinned by version and digest;
   - bridge registrations loaded from tuwunel's appservice directory;
   - one double-puppet registration shared by all bridges;
   - federation off, and no open registration.
3. **Setup checks.** Starting fails without an explicit server name. Generated secrets (`as_token`, `hs_token`,
   the bridge's `pickle_key`) go into git-ignored files, never into the repository or the logs.
4. **Bridge configuration template:** encryption allowed, on by default and required, sync-based, with
   `pickle_key: generate`.
5. **`NOTICE`** naming each bridge, its licence and its source (ADR 0002).
6. **`docs/deploy.md`:**
   - setup, and the permanent server name;
   - adding users;
   - what the volumes contain;
   - registering a Telegram `api_id`.
7. **`docs/backup.md`:** back up and restore, tested by restoring into a throwaway stack. Say plainly that the
   archive is a secret.
8. **`cargo xtask bump-images`:** move the pinned image digests forward and report stale ones.

## Milestone 2: the client

Goal: sign in, see the inbox, read and send text, with end-to-end encryption and a recovery key, on every
desktop target.

1. **Workspace:**
   - the crates and layers from ADR 0009;
   - `rust-toolchain.toml` and `deny.toml`;
   - `cargo xtask ci` and `cargo xtask layers`;
   - craft-libs pinned at one revision.
2. **CI:** a workflow calling `craft-common-actions`, in its own pull request (house rule 11).
3. **Spike: targets.**
   - Does `matrix-sdk`'s `aws-lc` TLS build on FreeBSD, riscv64 and Windows arm64?
   - Does `keyring` work on FreeBSD and inside Flatpak?
   - Write down the answer and any workaround.
4. **`pingcraft-core` model:** network, account, chat, inbox and message, with tests.
5. **Sign-in:**
   - Password sign-in and session restore in `pingcraft-matrix`.
   - Stores encrypted with a key kept in the OS keychain, never a plain file (ADR 0004).
6. **Inbox:** `SyncService` and `RoomListService` mapped onto the core model.
7. **Chat:** the timeline, reading and sending text.
8. **Encryption:**
   - cross-signing, key backup, and the recovery key screen;
   - verifying another device by emoji (SAS).
9. **UI:** sign-in, inbox and chat screens in `pingcraft-ui-egui`, with i18n and `egui_kittest` AccessKit tests.
10. **Packaging for every target in ADR 0009.** Releases are not published without the maintainer
    (house rule 9).

## Milestone 3: connecting networks from inside PingCraft

Goal: a user connects Telegram, Signal and WhatsApp accounts from PingCraft, after seeing each network's risk.

1. **The bridge provider interface** in `pingcraft-core` (ADR 0003).
2. **Spike: OpenID with federation off.** Does tuwunel serve the OpenID userinfo endpoint the bridges use to
   check a token when federation is off? Stop and ask the maintainer if it does not (ADR 0003).
3. **Route each bridge's `/_matrix/provision`** through the reverse proxy, and document it.
4. **The mautrix bridge provider:** uses the bridgev2 provisioning API, authenticated with a Matrix OpenID token.
5. **Network of a chat:** read from the bridge-info room state, and shown in the inbox.
6. **Risk screen:** shown before connecting, with an explicit confirmation. The text is written in our own words
   from ADRs 0005–0008.
7. **Connect Telegram** from inside PingCraft. **(maintainer: test account)**
8. **Signal:** bridge in compose, and connected from PingCraft. **(maintainer: test account)**
9. **WhatsApp:** bridge in compose, connected from PingCraft, with the bridge's "phone offline" warning shown in
   the inbox. **(maintainer: test account)**
10. **Accounts screen:** list a user's accounts and remove one.

## Milestone 4: web build and the CraftCloud option

Goal: the same client in a browser, served by CraftCloud.

1. **Spike:** `matrix-sdk-ui` with the encrypted IndexedDB store, running in a browser. Upstream only
   compile-checks it on wasm.
2. **`pingcraft-web`:** the Trunk build, with the unlock passphrase screen (ADR 0004).
3. **CraftCloud integration:** `craft-web-host`, and a release asset in the shape CraftCloud's `apps.toml`
   expects.
4. **`docs/craftcloud.md`:** a separate compose project, the homeserver's hostname behind the same reverse proxy,
   and `.well-known` delegation.
5. **The `apps.toml` entry in CraftCloud.** **(maintainer: CraftCloud repository)**

## Milestone 5: Discord

Starts when mautrix-discord releases its bridgev2 rewrite (ADR 0008).

1. Discord bridge in compose.
2. Bot login as the default, connected from PingCraft.
3. User login behind the high-risk warning. **(maintainer: test account)**

## Milestone 6: polish

1. Desktop notifications.
2. Search.
3. Media: images and files, over authenticated media.
4. Reactions, edits and replies.
5. Inbox filters by network and account.

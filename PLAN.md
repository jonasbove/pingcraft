# PingCraft plan

Words follow [`GLOSSARY.md`](GLOSSARY.md). Decisions live in [`docs/adr/`](docs/adr/).

Each ticket below is sized for one agent session. Tickets marked **(maintainer)** need something only the
maintainer can give: an account to test with, an approval, or a change in another repository. Bridge tests use
only accounts the maintainer provides for them; never another real account or phone number. Test notes, logs and
ADR addenda never name the account, its number or its chats. A test ends by logging the bridge out of the network with
the bridge's own logout, before the stack is deleted, so the network session ends with it.

## Phase 0: research and decisions (done)

Decided with the maintainer on 2026-11-24:

| ADR | Decision |
|---|---|
| [0001](docs/adr/0001-homeserver-tuwunel.md) | tuwunel is the homeserver, confirmed by a spike in milestone 1 |
| [0002](docs/adr/0002-agpl-bridges-at-arms-length.md) | the AGPL bridges run unmodified at arm's length |
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

1. **Done (2026-11-24), see ADR 0001's addendum. Spike (maintainer: a Telegram account and an `api_id`).** Run tuwunel and mautrix-telegram with
   Matrix-side encryption required. Log in to the bridge, then send and receive text both ways from a stock
   Matrix client. Measure idle and active memory of tuwunel and the bridge (`docker stats`), then stop the
   containers. Record the findings as an addendum to ADR 0001, and stop if tuwunel cannot do it.
2. **Done.** **`compose.yaml`** for the `pingcraft` project:
   - tuwunel, Postgres for the bridges, and the Telegram bridge;
   - images pinned by version and digest;
   - bridge registrations loaded from tuwunel's appservice directory;
   - one double-puppet registration shared by all bridges;
   - federation off, and no open registration.
3. **Done.** **Setup checks.** Starting fails without an explicit server name. Generated secrets (`as_token`, `hs_token`,
   the bridge's `pickle_key`) go into git-ignored files, never into the repository or the logs.
4. **Done.** **Bridge configuration template:** encryption allowed, on by default and required, sync-based, with
   `pickle_key: generate`.
5. **Done.** **`NOTICE`** naming each bridge, its licence and its source (ADR 0002).
6. **Done.** **`docs/deploy.md`:**
   - setup, and the permanent server name;
   - adding users;
   - what the volumes contain;
   - registering a Telegram `api_id`.
7. **Done.** **`docs/backup.md`:** back up and restore, tested by restoring into a throwaway stack. Say plainly that the
   archive is a secret.
8. **Moved to milestone 2, ticket 1** (it needs the Rust workspace). **`cargo xtask bump-images`:** move the pinned image digests forward and report stale ones.

## Milestone 2: the client

Goal: sign in, see the inbox, read and send text, with end-to-end encryption and a recovery key, on every
desktop target.

1. **Done.** **Workspace:**
   - the crates and layers from ADR 0009;
   - `rust-toolchain.toml` and `deny.toml`;
   - `cargo xtask ci` and `cargo xtask layers`;
   - `cargo xtask bump-images`, from milestone 1: move the pinned image digests forward and report stale ones;
   - craft-libs pinned at one revision.
2. **Done, not run yet** (it runs on GitHub once pushed). **CI:** a workflow calling `craft-common-actions`, in
   its own pull request (house rule 11): `ci.yml` runs `cargo xtask ci` through the shared `gate.yml`, on Linux
   (Windows runners have no system SQLite to link), and `house-rules.yml` checks `AGENTS.md`. The tests that need
   a homeserver skip there; a job that starts a throwaway tuwunel for them is still open.
3. **Done, see ADR 0009's addendum.** **Spike: targets.**
   - Does `matrix-sdk`'s `aws-lc` TLS build on FreeBSD, riscv64 and Windows arm64?
   - Does `keyring` work on FreeBSD and inside Flatpak?
   - Write down the answer and any workaround.
4. **Done.** **`pingcraft-core` model:** network, account, chat, inbox and message, with tests.
5. **Done.** Password sign-in, session restore and sign-out, with the store key and session in the keychain, tested
   against tuwunel. Without a keychain, the same secrets go in a `PassphraseVault`: one file encrypted with
   XChaCha20-Poly1305 under a key derived from the unlock passphrase with Argon2id. The store key itself stays
   random and is kept in the vault, rather than derived from the passphrase directly, so a passphrase can later be
   changed without re-encrypting the stores. The craft-libs proposal is drafted in
   [`docs/proposals/craft-libs-secrets.md`](docs/proposals/craft-libs-secrets.md), not opened anywhere. The unlock
   screen is ticket 9's. **Sign-in:**
   - Password sign-in and session restore in `pingcraft-matrix`.
   - Stores encrypted with a key kept in the OS keychain, or derived from an unlock passphrase where there is no
     keychain; never a plain file (ADR 0004).
   - Draft a proposal for a craft-libs secrets crate from this code (house rule 16). Nothing is opened on GitHub
     without the maintainer.
6. **Done.** **Inbox:** `SyncService` and `RoomListService` mapped onto the core model, over simplified sliding
   sync: name, unread count, last-message preview, encryption, and the network from bridge-info state (a second,
   small sliding sync connection asks for it, since the room list's required state is fixed). Tested against
   tuwunel. Still open: `Chat.account` is always `None` until milestone 3's bridge provider knows the accounts.
7. **Done.** **Chat:** the timeline, reading and sending text: `matrix-sdk-ui`'s `Timeline` mapped onto core messages
   (with "waiting for the key" for undecryptable ones and the send state), a failed message sent again, and older
   messages loaded back to the start of the chat. Tested against tuwunel.
8. **Done** (the screens with ticket 9). **Encryption:**
   - cross-signing, key backup, and the recovery key screen;
   - verifying another device by emoji (SAS).

   The first sign-in sets up cross-signing (with the password when the server asks for it) and a key backup with
   a new recovery key; a device that finds an existing backup asks for its recovery key and never replaces it. A
   new device can be verified from an old one by comparing symbols, in both directions. The core model
   (`pingcraft_core::encryption`) is what the interface renders. Tested against tuwunel with fresh users. Open,
   for the maintainer: what to offer when the recovery key is lost (resetting it loses the encrypted history);
   an account with a key backup but no secret storage (made by another client); verifying other people; QR codes.
9. **Done, with gaps below.** **UI:** the window and app icon from `design/assets/logo/`, the orchid theme from `design/tools/lib.py`, and the sign-in, inbox and chat screens in `pingcraft-ui-egui`, with i18n and `egui_kittest` AccessKit tests.

   Built: the sign-in, unlock (and choosing a passphrase), inbox, chat (every message state, retry, older
   messages), recovery key, entering the recovery key and verify-device sheets, over
   `pingcraft_core::app::Backend`; the desktop app runs them on `pingcraft_matrix::engine`, which is tested
   end to end against tuwunel. Open:
   - **i18n:** `craft-i18n` cannot be linked next to `matrix-sdk` 0.19.1: its ICU crates turn on `zerovec`'s
     `serde` feature, and `ruma-common` 0.20.2 then fails to compile (E0275 on `Box::deserialize`). The UI uses a
     stand-in with the same call shape (`tl!`, `fmt`, `trn`) and no catalogs until ruma or craft-i18n changes.
   - **Fonts:** Archivo, Inter and Geist Mono are not bundled (house rule 3 puts fonts in craft-fonts, and
     PingCraft does not use it yet); egui's own faces stand in. egui's emoji face lacks some of the SAS symbols.
   - **Design gaps** (built in the design's language, for the maintainer to review): choosing a passphrase, the
     "this device can't read earlier messages" notice, entering the recovery key, and a confirmation before
     signing out from the unlock sheet clears this computer. White on the orchid (unread counts) is 4.47:1, a
     hair under the 4.5:1 the design states.
   - **Not built yet:** the ACCOUNTS and SETTINGS tabs, the network filter (a later decision), the chat header's
     account line and "…" menu, SAVE TO FILE for the recovery key, deleting a failed message, and the window's own
     title bar (the system's is used).
10. **Scripts done; most targets build only in CI, not run yet.** **Packaging for every target in ADR 0009.** Releases are not published without the maintainer
    (house rule 9).

    `packaging/` follows CraftCloud's (itself craft-launcher's), for craft-common-actions' shared release
    workflow: Linux (AppImage, deb, rpm, tar.gz, Flatpak; the shared workflow makes the Arch package from the
    tarball), FreeBSD (tar.gz), macOS (universal dmg) and Windows (msi, portable zip), with the icons from
    `design/assets/logo/`, plus a Nix flake and the `cargo xtask version set` the shared workflow calls.
    `.github/workflows/release.yml` runs them by hand as a dry run
    only: nothing is signed, drafted or uploaded. Every package is checked for a bundled SQLite older than
    3.54.0 (`check_sqlite`); Linux, FreeBSD and macOS link the system's SQLite, and the Windows build compiles
    the pinned SQLite 3.54.1 amalgamation in. Built here: the Linux x86_64 AppImage, deb, rpm and tar.gz, and
    the FreeBSD layout (`--dry-run`). Open:
    - the deb and rpm depend on the system SQLite without a minimum version: requiring 3.54.0 would make them
      uninstallable on today's stable distributions (Ubuntu 24.04 has 3.45);
    - the logo drawing's origin and licence are still to be confirmed (`ATTRIBUTION.md`), and every package
      now carries the logo;
    - `LICENSE-MIT` and `LICENSE-APACHE` were added for the packages ("the PingCraft contributors", as
      CraftCloud's), for the maintainer to confirm;
    - Windows, macOS, FreeBSD, riscv64, Flatpak and the Nix flake are untried until CI runs them; there is no
      `flake.lock` yet.
    - craft-common-actions' release workflow installs no SQLite to link against (no `libsqlite3-dev` on Linux and
      riscv64, no `sqlite3` package on FreeBSD), so those jobs will fail to link until it does: a change for
      craft-common-actions, not this repository (house rule 11).
11. **Ignore accepted by the maintainer (2026-11-24); milestone 2 does not wait on it.** **SQLite advisory RUSTSEC-2026-0459.** `matrix-sdk` 0.19.1 locks a SQLite binding older than the fix, and
    `deny.toml` carries it as a reasoned ignore. Move to the first `matrix-sdk` release on `rusqlite` 0.41 or newer,
    drop the ignore, and make sure no package ships a bundled SQLite older than 3.54.0.

## Milestone 3: connecting networks from inside PingCraft

Goal: a user connects Telegram, Signal and WhatsApp accounts from PingCraft, after seeing each network's risk.

1. **The bridge provider interface** in `pingcraft-core` (ADR 0003).
2. **Spike: OpenID with federation off.** Does tuwunel serve the OpenID userinfo endpoint the bridges use to
   check a token when federation is off? If not, make only that endpoint reachable to the bridges over the
   internal compose network (ADR 0003).
3. **Route each bridge's `/_matrix/provision`** through the reverse proxy, and document it.
4. **The mautrix bridge provider:** uses the bridgev2 provisioning API, authenticated with a Matrix OpenID token.
5. **Network of a chat:** read from the bridge-info room state, and shown in the inbox.
6. **Risk screen:** shown before connecting, with an explicit confirmation. The text is written in our own words
   from ADRs 0005–0008.
7. **Connect Telegram** from inside PingCraft. **(maintainer: an account)**
8. **Bridge in compose: done (setup, NOTICE, docs/deploy.md section 8). Open: connecting from PingCraft.** **Signal:** bridge in compose, and connected from PingCraft. **(maintainer: an account)**
9. **WhatsApp:** bridge in compose, connected from PingCraft, with the bridge's "phone offline" warning shown in
   the inbox. **(maintainer: an account)**
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
3. User login behind the high-risk warning. **(maintainer: an account)**

## Milestone 6: polish

1. Desktop notifications.
2. Search.
3. Media: images and files, over authenticated media.
4. Reactions, edits and replies.
5. Inbox filters by network and account.

## Later, not yet planned

- **Network colours and the network filter in the inbox** (maintainer, 2026-11-24; in the design as
  `inbox-networks`, not built). Each network's tag gets its own colour, and a row of toggles hides or shows each
  network's chats. Open questions before it becomes tickets, to grill the maintainer on:
  - whether hiding a network is remembered per device, per user, or only for the session;
  - whether hidden networks still count towards unread totals and notifications;
  - one toggle per network, or per account (two Telegram accounts);
  - how the toggles behave with many networks, and with colour-blind users (the names stay, so colour is never the
    only cue).

- **MailCraft integration** (raised by the maintainer, 2026-11-24). What it means is still open: for example,
  mail as one more network in the inbox, or PingCraft chats shown in MailCraft. It needs its own ADR before any
  ticket.

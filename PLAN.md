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
- **The shared agent rules** (craft-contrib RFC-0072, adopted 2026-11-26 because the House rules check requires
  them): `AGENTS.md` now carries the suite's `craft-agents` block in place of the retired house-rules blocks, and
  "house rule N" elsewhere in this plan refers to the old numbering. Where PingCraft does not follow the new rules
  yet: integration tests are several binaries in `crates/matrix/tests/` rather than one `tests/it/main.rs` per
  crate (rule 30); there is no automation interface or `pingcraft-cli mcp` (rules 14 and 18); interface text goes
  through a stand-in for craft-i18n, which now links next to matrix-sdk but is not swapped in yet (rule 16; M2
  ticket 9); colours, type sizes, radii and control heights come from craft-ui's theme since the craft-libs update,
  part 1, but panel heights, margins, widths and emoji symbol sizes are still literals in the code (rule 14).
- **Git dependencies** (maintainer, 2026-11-26): any `github.com/storytold` repository may be a cargo git source
  (for example `storytold/vectorcraft`, which craft-libs' `craft-svg` pulls in through `craft-ui`), always pinned
  by `rev`. The change that first needs a storytold source not yet in `deny.toml` replaces the per-repository
  `allow-git` entries with `[sources.allow-org] github = ["storytold"]`, with a comment saying why. Git sources
  outside storytold stay denied.

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
   - **Pins** (2026-11-26): craft-libs `a6103343`, the egui family on the storytold fork at `b9770908`, winit at
     `a517b384`. Moving them is a change of its own (rule 10).
   - **Patched ruma, carried until upstream fixes it** (maintainer, 2026-11-26): `vendor/ruma-common-0.20.2` and
     `vendor/ruma-events-0.35.2`, the crates.io releases with four `deserialize` calls typed out, through
     `[patch.crates-io]`. Without it craft-libs (craft-settings → craft-host → craft-i18n, whose ICU crates turn
     on `zerovec/serde`) and matrix-sdk 0.19.1 do not compile together (E0275). `vendor/README.md` lists every
     changed line. **Removed** once a ruma release has the fix and matrix-sdk depends on it (or craft-i18n stops
     turning on ICU's `serde` features): delete the folders, the patch entries, the workspace `exclude` and the
     `ATTRIBUTION.md` rows. No upstream issue or fix has been sent: ruma accepts no LLM-written contributions
     (code, issues or text) and matrix-rust-sdk no agent-written ones and asks for a DCO sign-off, so the
     maintainer writes them himself. matrix-sdk's widget code has the same pattern but is not compiled here
     (`experimental-widgets` is off).
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
9. **Done, with gaps below.** **UI:** the window and app icon from `design/assets/logo/`, the orchid theme from `design/tools/lib.py` (replaced by craft-ui's default theme in the craft-libs update, part 1), and the sign-in, inbox and chat screens in `pingcraft-ui-egui`, with i18n and `egui_kittest` AccessKit tests.

   Built: the sign-in, unlock (and choosing a passphrase), inbox, chat (every message state, retry, older
   messages), recovery key, entering the recovery key and verify-device sheets, over
   `pingcraft_core::app::Backend`; the desktop app runs them on `pingcraft_matrix::engine`, which is tested
   end to end against tuwunel. Open:
   - **i18n:** `craft-i18n` links next to `matrix-sdk` 0.19.1 since the craft-libs update (with the patched ruma
     in `vendor/`, M2 ticket 1), but the UI still uses its stand-in with the same call shape (`tl!`, `fmt`,
     `trn`) and no catalogs. The swap is its own change, not a drop-in: craft-i18n's `trn` writes numbers grouped
     for the region ("1,234 chats"), `fmt` wraps inserted values in bidi isolates when there is right-to-left
     text, and catalogs need a `build.rs` (`craft-i18n-build`) and an `I18n` with the language setting.
   - **Fonts: craft-ui's font stack** (craft-libs update, part 1, 2026-11-26; maintainer: craft-libs dictates the
     look). The interface draws with `craft_ui::fonts::UiFonts`: craft-ui's Inter (it ships the face itself) ahead
     of egui's faces, installed system fonts on the desktop for scripts those lack (`system_fonts`; the tests turn
     them off so they draw the same everywhere), and egui's monochrome emoji as the last resort. The design's
     Archivo and Geist Mono are gone with PingCraft's own look. No craft-fonts faces are embedded yet
     (`craft-fonts-build`'s `Embed`, a craft-fonts checkout in `CRAFT_FONTS_DIR`): the web build will need them for
     other scripts, since it has no system fonts.
   - **Design gaps** (built in the design's language, for the maintainer to review): choosing a passphrase, the
     "this device can't read earlier messages" notice, entering the recovery key, and a confirmation before
     signing out from the unlock sheet clears this computer. (Before part 1:) White on the orchid (unread counts) was 4.47:1, a
     hair under the 4.5:1 the design states.
   - **Not built yet:** the SETTINGS tab (ACCOUNTS came with milestone 3), the network filter (a later decision), the chat header's
     account line (its "…" menu came with "Messages and chats"), SAVE TO FILE for the recovery key, deleting a failed message, and the window's own
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
    - the deb and rpm require the system SQLite 3.54.0 or newer (maintainer, 2026-11-25: correctness over
      installability), so they cannot be installed on today's stable distributions until their SQLite moves:
      Ubuntu 24.04 has 3.45. The rpm names the package (`sqlite-libs` on Fedora/RHEL, `libsqlite3-0` on openSUSE),
      since the soname has no version. Not built here (no nfpm on this machine);
    - the logo drawing: generated with an AI image tool by the maintainer for PingCraft, under the repository's
      licence (maintainer, 2026-11-25; `ATTRIBUTION.md`);
    - `LICENSE-MIT` and `LICENSE-APACHE` were added for the packages ("the PingCraft contributors", as
      CraftCloud's); confirmed by the maintainer, 2026-11-25;
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

1. **Done.** **The bridge provider interface** in `pingcraft-core` (ADR 0003): `pingcraft_core::bridge`, with
   networks and their login flows, the login steps (show and wait, ask, not supported yet, done), accounts and
   removing one, behind `dyn BridgeProvider`.
2. **Done (2026-11-24), see ADR 0003's addendum.** **Spike: OpenID with federation off.** Does tuwunel serve the OpenID userinfo endpoint the bridges use to
   check a token when federation is off? If not, make only that endpoint reachable to the bridges over the
   internal compose network (ADR 0003).

   tuwunel serves it, but the bridges reach it only the federation way: HTTPS to the server name, never at a
   private address. The fix, built with ticket 3: inside the stack, the reverse proxy answers to the server name with
   a certificate the bridges trust, on a network outside RFC 1918, and forwards only that endpoint. The upstream
   proposal (a bridge checking its own server's tokens at `homeserver.address`) is dropped and the workaround
   stays (maintainer, 2026-11-25; ADR 0003's second addendum).
3. **Done.** **Route each bridge's `/_matrix/provision`** through the reverse proxy, and document it.
   A Caddy proxy is now the stack's one way in (`deploy/Caddyfile`, docs/deploy.md section 7): the homeserver, and
   each bridge's `/_matrix/provision/v3/` under `/_pingcraft/bridges/<network>/`, nothing else of a bridge. Inside
   the stack it also answers the bridges' OpenID check (ticket 2). The WhatsApp bridge is in compose too (setup,
   NOTICE, docs/deploy.md section 10), without a login. Checked on a throwaway stack: `whoami` with an OpenID
   token through the proxy answered for all three bridges; other bridge paths, a path with `..`, no token and
   another user's ID were refused.
4. **Done.** **The mautrix bridge provider:** uses the bridgev2 provisioning API, authenticated with a Matrix OpenID token.
   `pingcraft_matrix::mautrix` (v3: whoami, login start, step, cancel, logout) and the engine's commands for it.
   Steps PingCraft cannot do yet (cookies, webauthn, a choice from a list) end as "not supported". Desktop only
   until the web build. Tested live on a throwaway stack (`tests/bridges.rs`, `PINGCRAFT_TEST_BRIDGES=1`): Signal
   and WhatsApp showed a QR code, Telegram and WhatsApp asked for a phone number, each was cancelled, and no
   account was made.
5. **Network of a chat:** read from the bridge-info room state, and shown in the inbox.
6. **Done.** **Risk screen:** shown before connecting, with an explicit confirmation. The text is written in our own words
   from ADRs 0005–0008. Built from `connect-risk`: the risk scale, the blocks for Telegram, Signal and WhatsApp, and
   CONNECT stays off until the box is ticked. Then the bridge's login steps: choosing a way to log in when there are
   several, a QR code (drawn with the `qrcode` crate, MIT OR Apache-2.0), a code, questions, and "not supported
   yet". Differs from the design: WhatsApp's "keep your phone online" says the bridge warns at 12 days, since
   PingCraft's own inbox warning is ticket 9 (wording confirmed by the maintainer, 2026-11-25).
7. **Connect Telegram** from inside PingCraft. **(maintainer: an account)**
8. **Bridge in compose: done (setup, NOTICE, docs/deploy.md section 9). Open: connecting from PingCraft.** **Signal:** bridge in compose, and connected from PingCraft. **(maintainer: an account)**
9. **Bridge in compose: done with ticket 3 (docs/deploy.md section 10). Open: connecting from PingCraft, and the
   warning.** **WhatsApp:** bridge in compose, connected from PingCraft, with the bridge's "phone offline" warning shown in
   the inbox. **(maintainer: an account)**
10. **Done.** **Accounts screen:** list a user's accounts and remove one.
    The header's INBOX and ACCOUNTS tabs (and SETTINGS, M3 ticket 11); the network cards from
    `connect-network`, with each card's accounts and whether its bridge answers; the user's accounts with their state
    and REMOVE, which asks first; and "Connect a network" from the empty inbox. Not checked against a real account
    (tickets 7 to 9), and the screens were checked through their AccessKit tree only, not by eye.
11. **Built (2026-11-26), with gaps; no frame yet** (it follows craft-libs' preferences pattern, so the design file
    needs a `settings` frame drawn after it). The SETTINGS tab, on `PreferencesPage` (craft-libs update, step 5):
    - **Appearance:** light, dark, or as the system is (default), as craft-theme's mode, kept on this computer
      (`[appearance] mode = "light" | "dark" | "system"` in craft-settings' `config.toml`: on Linux
      `$XDG_CONFIG_HOME/pingcraft/config.toml`, or `$PINGCRAFT_CONFIG_DIR/config.toml`), also from View, Appearance in
      the app menu. It is read and handed to the runtime before the first frame (`PingCraftApp::prime`; tested), so a
      dark choice never opens on a light frame. "As the system is" follows craft-platform's appearance reader, which
      `framework::install` starts on the desktop (the XDG portal's `color-scheme` on Linux) and which wakes the window
      when it changes; not tried by hand. A footnote says colour themes are not there yet (M6 ticket
      7 adds them on this page).
    - **Notifications:** message text in notifications on or off (off: "New message from …"), kept with the account
      (see "Questions for the maintainer"; this computer keeps a copy in `[notifications] previews`); and each network's notification sound and accent colour (moved here from the
      ACCOUNTS tab, which now points here), the colour any colour from craft-color-ui's colour picker
      (`ColorPickerDialog`, eyedropper off) or typed as hex, kept with the account as before.
    - **Emoji:** the default skin tone, the same synced setting as the picker's.
    - **Security:** this device's name (renamed when the field is committed); the unlock passphrase, changed in a
      sheet that asks for the current one and the new one twice (only where PingCraft uses one; with the keychain the
      row says there is none); sign out of this device, in a sheet that warns when no recovery key is set up; the
      recovery key's state, with "Create a new recovery key…" when this device can open the backup or there is none
      (a sheet asks first, then the key is shown once in the first sign-in's sheet, captioned "New recovery key"), or
      "Enter recovery key…" and "Verify this device" when it cannot; and the other devices, each with its name, when
      it was last seen and whether it is verified, with Verify (the usual verification sheet) and Sign out, which asks
      for the account password (the server's interactive authentication; a server that asks for something else gets
      "PingCraft cannot do that yet").
    - **Gaps:** Reset Section shows on every page (craft-libs gives no way to leave it out); on Security it does
      nothing, since nothing there has a default. Not looked at by eye, and no live test ran.
    The original list, for reference:
    - **Appearance:** dark or light theme.
    - **Passphrase:** change the passphrase that unlocks PingCraft on this device (ADR 0004).
    - **Recovery key:** whether recovery is set up, and making a new key, shown once as on first sign-in.
    - **This device:** its name as other devices see it, editable.
    - **Your other devices:** each signed-in device with its name, last seen and whether it is verified; verify one,
      or sign it out.
    - **Sign out** of this device, with a warning when the recovery key is not set up, since this device's message
      keys would then be lost.
12. **History as an opt-in for each connection: not doing it for now** (maintainer, 2026-11-25, after a spike).
    PingCraft imports no history; see "Chat history" under "Later, not yet planned" for why and what would change it.

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

1. **Each network described in one place** (maintainer, 2026-11-25). Today a network's facts are spread
   over six places: the bridge list (`crates/matrix/src/mautrix.rs`),
   the risk level (`crates/core/src/lib.rs`), the display name (`widgets.rs`), the colour (`theme.rs`), and the
   card list and login text (`accounts.rs`). Gather them into one description per network (id, name, icon,
   colour, risk level and its ADR, how to log in, the card's text), read by core and the UI, so adding Discord,
   and later networks, is mostly adding one entry. Whether it is Rust data or a file shipped with the app is
   decided in the ticket; it is never downloaded at run time.
2. Discord bridge in compose.
3. Bot login as the default, connected from PingCraft.
4. User login behind the high-risk warning. **(maintainer: an account)**

## Milestone 6: polish

1. **Desktop notifications** (maintainer, 2026-11-25), each operating system its own way, behind one small
   interface (`pingcraft_core::notify::Notifier`) with a fake for tests, in a crate only the desktop app uses:
   - **Linux and the BSDs:** the freedesktop notification spec over D-Bus (`org.freedesktop.Notifications`),
     through a pure-Rust D-Bus client (zbus), so it needs no libdbus and works on any init system (systemd, runit,
     OpenRC, s6), without systemd or logind. Without a notification service it says so once in the log and goes on.
   - **macOS:** the native user notification center (`UNUserNotificationCenter`), which shows notifications only
     for an app in a bundle with a bundle identifier, and after the user allows them.
   - **Windows:** native toast notifications (WinRT), which need the app's AppUserModelID on a Start menu shortcut
     (the installer's job).
   - **Web:** nothing for now (the browser's Notification API later); the web build still compiles.

   What notifies: a new message from someone else in a chat that is not muted, unless that chat is open and the
   window has focus. The title is the chat's name, the body the sender and a short preview; message text never goes
   in a log. Clicking a notification opens its chat where the platform reports clicks. A setting to leave previews
   out is on the Settings screen's Notifications page (done 2026-11-26; kept on this computer).

   **Built (2026-11-25), checked on Linux only.** `pingcraft-notify`: zbus 5 (already in the tree through the
   keyring) on Linux and the BSDs, with clicks from the `default` action; notify-rust 4.19 (MIT OR Apache-2.0) on
   macOS (its `preview-macos-un` backend, which asks for permission at the first notification) and Windows
   (tauri-winrt-notification, under the AppUserModelID the MSI's Start menu shortcut already carries). Both the macOS and
   the Windows backends stay (confirmed by the maintainer, 2026-11-25). The Flatpak
   may now talk to `org.freedesktop.Notifications` (permission kept, confirmed by the maintainer, 2026-11-25). The decision (which changes notify, and the text) is in
   `pingcraft-ui-egui` and tested on its own and through the AccessKit tests with a fake notifier. craft-libs has no
   notification API yet (its `platform` crate uses zbus for appearance only); this crate is a candidate for one.
   Open:
   - **A hidden window may notify late.** The decision runs in the interface's frame, which the engine's wake asks
     for; a window that is minimised or on another workspace may get no frames (Wayland compositors hold back frame
     callbacks), so its notifications would wait until it is shown again. Not checked; moving the decision next to
     the engine would fix it.
   - Starting PingCraft after messages arrived while it was closed shows one notification per chat with new ones.
   - **Not tried against a real notification service,** on any system: only built, and on Linux tested without a
     bus (it says so once and shows nothing).
   - **macOS was not compiled** (no macOS target here); **Windows** was only checked (`cargo clippy` for
     `aarch64-pc-windows-msvc`), not built or run; FreeBSD was checked the same way.
   - **The portable Windows zip** (maintainer, 2026-11-25): on its first run PingCraft makes its own Start menu
     shortcut carrying the AppUserModelID when neither the user's nor the machine's Start menu has one
     (`pingcraft_notify::shortcut`; the `.lnk` is written byte by byte, so no COM and no `unsafe`). Checked with
     `cargo clippy` for `aarch64-pc-windows-msvc` and unit tests of the file's layout on Linux; **untested on
     Windows**: whether Windows accepts the shortcut and then shows toasts is not known. A failure is said once on
     stderr and PingCraft goes on.
   - The web build shows none (the browser's Notification API later).
2. Search.
3. Media: images and files, over authenticated media.
4. Reactions, edits and replies: now part of "Messages and chats" below.
5. Inbox filters by network and account.
6. **Unread count on the app's icon** (maintainer, 2026-11-25): the total unread on the taskbar or dock
   icon, or in the tray where there is one, cleared as chats are read. Per platform: what each desktop supports
   (Linux launchers vary) is checked in the ticket.
7. **Colour themes** (maintainer, 2026-11-26), on craft-libs' `craft-theme` and `craft-theme-store`, after the
   craft-libs update moves PingCraft onto craft-ui:
   - **PingCraft opens in craft-libs' default theme** (maintainer, 2026-11-26): the orchid look is dropped, not
     kept as a theme, under the rule that craft-libs dictates the look. Network colours stay the networks' own in
     every theme. The Settings screen's Appearance page is where the theme choice goes (part 2); today it holds the
     mode (light, dark, or the system's).
   - **Built in, in PingCraft only** (not craft-libs built-ins): Tokyo Night and Catppuccin (Latte, Frappé,
     Macchiato, Mocha), as `craft-design-theme` files from their MIT-licensed palettes, each in `ATTRIBUTION.md`;
     alongside craft-libs' own built-ins.
   - **Gruvbox goes to craft-libs** (maintainer, 2026-11-26): proposed as the built-in `craft.gruvbox` (light and
     dark) in storytold/craft-libs#3701. PingCraft carries it itself only if #3701 is not merged by the time the
     themes are built.
   - **Import:** craft theme JSON (craft-theme-store already does this), base16 and base24 schemes (one converter
     onto the theme's seeds), and VS Code colour themes (a best-effort mapping; the result is checked by craft-theme's
     contrast pass and the user sees what could not be mapped).
   - **One choice for the Craft suite,** shared with the other Craft apps, with a "just for PingCraft" option
     (craft-theme's per-app choice).

## Messages and chats (maintainer, 2026-11-25)

Frames: page "Messages · Draft" in `design/pingcraft.figcraft` (draft for review, `design/README.md`), redrawn on
2026-11-25 to match what is built, including the differences listed next; the ACCOUNTS screens have frames as built
on page "M3 · Accounts". The message
menu is egui's own popup: craft-libs `menu` is the window's menu bar, not a context menu. Its keyboard shortcuts
(through `craft-keymap`) are still to come. Differs from the draft frames, for the maintainer's review: there is no
bar on hover (the "…" button is always there, so a keyboard and a screen reader reach it), and the quick reactions
are in the menu; menu items have no icons or keys yet; the pinned bar steps through the pins with a "next" button
instead of ALL PINNED; the forward sheet picks one chat with radio buttons; the typing line sits over the composer
instead of under the last message.

What people expect from a chat app beyond sending text. It does not wait on Discord; where it goes among the
milestones is the maintainer's call. Every screen here needs a frame in the design first (house rule), and each
ticket checks what the three bridges carry to and from each network: where a network cannot do something (for
example, editing on a network without edits), PingCraft says so instead of pretending, and what stays in
PingCraft only (local pins, mute, sounds) is labelled as such.

1. **Done on the desktop (2026-11-25), with gaps.** **Emoji, properly.** Colour emoji in messages, previews and names (egui shows most emoji as monochrome or
   missing glyphs today): a colour emoji font under an allowlisted licence (`OFL-1.1` or `CC-BY-4.0`, with the
   attribution), sequences such as skin tones, flags and joined emoji, and an emoji picker with search and recent
   emoji. Typing `:name:` to insert one.
   Built (maintainer's decisions 2026-11-25, `docs/proposals/emoji-font.md`): Noto Color Emoji COLRv1 v2.057
   (OFL-1.1; `ATTRIBUTION.md`, the licence in `NOTICE`), unmodified, through epaint's `color_fonts`. No font file
   is committed and the build downloads nothing: `cargo xtask fonts` fetches it from its pinned noto-emoji commit
   into the git-ignored `.cache/craft-fonts` (or `CRAFT_FONTS_DIR`) and checks its SHA-256, and
   `crates/ui-egui/build.rs` checks it again before embedding it. Without the file the build falls back to egui's
   monochrome emoji (the colour tests are then ignored, with the reason); `CRAFT_FONTS_REQUIRED=1` makes that an
   error, and `cargo xtask ci` (which fetches the font first) and the packaging scripts set it, so CI runs the
   colour tests. egui picks a face by a cluster's first character, so keycaps would start in the text face: emoji
   clusters (told apart with `unicode-segmentation` and the `emojis` data) are laid out in a family whose first face
   is the colour one, in messages, names, previews, quotes, reactions, the bars, the composer and the verification
   symbols; the text and accessible names are unchanged. Tests check that a joined family, a skin tone, a flag, a
   keycap and a red heart are each one colour glyph (COLRv1 gradients are painted). The picker (search by name and
   shortcode, recently used, every emoji) is behind an emoji button by the composer and MORE REACTIONS… in a
   message's menu; the quick reactions put recently used emoji first; `:name:` turns into the emoji, with
   suggestions while it is typed. Size, release build on Linux x86_64: 73.3 MB stripped before, 80.0 MB after
   (+6.7 MB; the font is 5.0 MB). Follow-ups (2026-11-26) and what is still open:
   - **The web build loads the colour font after the page opens** (maintainer, 2026-11-26). `color_fonts` builds
     for wasm32 (clippy `-D warnings`, as `cargo xtask ci` checks it), so it is no longer target-gated. The web build
     starts with egui's monochrome emoji, and `emoji::install` starts fetching `noto-color-emoji/Noto-COLRv1.ttf`
     next to the app (through `web-sys`, no JavaScript) without holding up the first frame; once it arrives,
     `emoji::install_color_font` checks it is the pinned file (size, then SHA-256, from `font_pin.rs`) and installs
     the font stack again with it (since the craft-libs update: craft-ui's `UiFonts` with the font as
     `Emoji::color` and at the head of the `emoji` family; `Emoji::served` cannot do this, see "craft-libs: what
     PingCraft lacks"). A failed fetch or any other file leaves emoji monochrome and says nothing. Tested natively: a context
     that starts monochrome draws 👍 in colour once the font is installed, and a wrong or too big file is refused.
     **Not run in a browser:** `pingcraft-web` is still the stub that exports nothing (no eframe app, no
     `index.html`, no Trunk), so the fetch has never run and the wasm size with the interface is still unknown.
     Milestone 4's Trunk build has to copy the pinned file from the fonts folder to that path (for example a
     `data-trunk rel="copy-file"` of `$CRAFT_FONTS_DIR/noto-color-emoji/Noto-COLRv1.ttf` into
     `noto-color-emoji/`) and ship its `OFL.txt`, as the desktop packages do.
   - **Recently used emoji are kept with the account** (maintainer, 2026-11-26), so they follow the user between
     devices and survive a restart. Format: the specification's `m.recent_emoji` global account data (added in
     Matrix v1.18, "Recently used emoji": `{"recent_emoji": [{"emoji", "total"}]}`, most recent first, at most 100),
     not `io.element.recent_emoji`: the brief expected no `m.` standard, but there is one, and Element Web now
     writes it too and reads its older `io.element.recent_emoji` (`[emoji, count]` pairs) only to merge in. PingCraft
     does the same: an account with only the older event starts from it, and only `m.recent_emoji` is written
     (`crates/matrix/src/recent_emoji.rs`). Each use (picked in the picker, a quick reaction, a suggested
     shortcode) moves the emoji to the front and adds one; uses are sent together 3 seconds after the last (and at
     once when PingCraft closes or signs out), on top of what the server has at that moment, fetched fresh, so
     another device's change is merged, not overwritten. Entries and keys PingCraft does not know are kept;
     malformed ones are dropped. Nothing is sent while the server cannot be reached; a failed send is tried again 30
     seconds later, quietly, and each request gives up after 5 seconds without retrying (12 for the whole send), so
     the engine's loop and closing the window never wait longer. Emoji just sent stay in the list until the sync
     brings the server's copy. The interface
     shows the 24 newest it knows and forgets them on sign-out. Tests: the merging, the order, the cap, the older
     format and the timing (unit tests), the interface's side through the AccessKit tree, and a live test with one
     user on two devices (`tests/recent_emoji.rs`), which **was not run against a server**: the test server's
     variables were not set in this session, so it skipped. That leaves the main link unproven: that the sync
     (simplified sliding sync's account data extension) brings `m.recent_emoji` into the store, which is what the
     interface shows on another device or after a restart. Not tried by hand with Element. Uses not yet sent when
     PingCraft closes offline are lost. The web build does not have it yet (the module is native only, like the
     engine). Typing a whole `:name:` (not picking the suggestion) does not count as a
     use, as before.
   - Emoji names and search are English.
   - **Skin tone: one saved default, kept with the account** (maintainer, 2026-11-26). A SKIN TONE row in the
     picker (a raised hand in each of the six tones, radio buttons to a screen reader) sets the tone for every emoji
     that has skin tones: the picker's grid, the fixed quick reactions and `:name:` shortcodes and their suggestions
     (emoji of two people take it for both; an emoji that already has a tone, such as a recently used one, keeps its
     own). **Synced between devices** (maintainer, 2026-11-26): it is PingCraft's own global account data,
     `io.github.jonasbove.pingcraft.emoji` with `{"skin_tone": "medium"}` (`crates/matrix/src/emoji_settings.rs`),
     not a key in `m.recent_emoji` (that content has only `recent_emoji` in the specification, and apps that read it
     into a typed struct, ruma's for one, would drop an extra key the next time they write). Once signed in, the
     engine asks the server itself which tone the account keeps (never concluding "none" from a sync that has not
     arrived), then watches the synced copy for another device's change; a tone chosen here is sent within a second,
     on top of the server's copy (other keys kept, through the shared `account_data.rs` helper that
     `m.recent_emoji` now uses too), again 30 seconds after a failure, and at once when PingCraft closes or signs out.
     Requests give up after 5 seconds (12 in all). The desktop app keeps a **local copy** in craft-settings'
     `config.toml` (`[emoji] skin_tone`), as the start-up value and for when the server cannot be reached.
     **Migration:** the first time an account answers, a local tone from before (set on this computer, account has
     none) is sent up once; `[emoji] skin_tone_migrated = true` then records that, so a later account signed in on
     this computer that keeps no tone starts from the default instead of inheriting it. Tests: the decision (pure,
     every case), the engine's state machine (ask first, retry, tell another device's change once, never echo
     its own choice, a choice made before the server answered wins), the interface through the AccessKit tree (an
     account's tone is marked in the picker and not sent back; a picked one is sent once; the move-up happens once),
     the settings file (the flag, and an older file without it), and a live test with one user on two devices
     (`tests/skin_tone.rs`), which **was not run against a server**: the test server's variables were not set in
     this session, so it skipped. Not tried by hand. The web build has no settings store and no engine yet, so there
     the tone lasts for the session.
   - **Frames as built** (2026-11-26): `emoji-picker`, `emoji-react` and `emoji-shortcode` on page "Messages ·
     Draft" in `design/pingcraft.figcraft` (`design/tools/messages.py`, exports at 1x and 2x), for the maintainer's
     review. Emoji are placeholder squares there and the tones swatches: FigCraft cannot paint the COLRv1 font.
   - Without the colour font (the fallback build, and the web until the font arrives) the picker shows tofu for
     emoji egui's monochrome face lacks; its three tests are ignored in that build.
   - Not tried by hand in the running app; checked through the tests only. A Nix build (no network in its
     sandbox) falls back to monochrome emoji.
   - **Emoji a size up, and large on their own** (maintainer, 2026-11-26). Emoji looked soft because they were drawn
     at the text's size (11–14 px; epaint's placement and rasterising were already pixel-exact). Emoji among text
     are now drawn at 1.3 times the text (`emoji::INLINE_SCALE`), as Signal does, in messages, names, previews,
     quotes, reactions, the bars and the composer; every line of text that has an emoji is 1.5 times the text size
     tall (`EMOJI_LINE`) with text and emoji centred in it, since egui otherwise makes only the rows with an emoji
     taller and lines jump. One line of text (names, previews, the header, quotes, the bars, reactions, the
     composer) keeps exactly the text's own height: the emoji is centred on it and reaches 1–3 px above and below,
     into the padding, so the inbox rows, the header and the composer do not move when an emoji appears. A message of one to three emoji and nothing but white space (`emoji::is_large`) is
     drawn at 2.5 times the body (`LARGE_SCALE`); four, or any text, is an ordinary message (Signal's limit is 5,
     Telegram Desktop's 3). Previews and names never go large. The picker, the quick reactions, the emoji button and
     the verification symbols keep the size they ask for (`emoji::symbol`). Tests: the sections' sizes and line
     heights, which messages are large, the glyphs fitting their lines in the real fonts (at 12.5 px an emoji is now
     about 18 px tall in a 19 px line, against a 14.4 px text line), a line with an emoji as tall as one without, the
     composer's height while an emoji is typed, and a large message through the AccessKit tree with its text
     unchanged. Open: a message with an emoji has its lines a third further apart than one without
     (the maintainer may prefer the emoji to overlap the line spacing a little instead).
   - **Symbols that are text by default follow the chat's network's app** (maintainer, 2026-11-26; it started with a
     ☺ that Signal shows as a colour emoji and PingCraft showed as text). `Network::emoji_presentation` in
     `pingcraft-core`: `Unicode` (text unless U+FE0F follows) or `EmojiFirst { arrows }` (an emoji whenever it has an
     emoji form, except ©, ® and ™; arrows such as ↔ only with `arrows`). Everywhere: U+FE0F makes an emoji, U+FE0E
     keeps text, digits, `#` and `*` are text unless a keycap. Telegram: `EmojiFirst { arrows: true }`; Signal and
     WhatsApp: `EmojiFirst { arrows: false }`; Matrix, Discord and networks not looked at: `Unicode`. Used for
     message bodies (and which are large), names, previews, quotes, reactions, the bars and the composer; the picker
     and suggestions show emoji PingCraft chose, with U+FE0F. Unit tests per network for ☺ ❤ ✔ © 1 ↔ and ☺︎, and a
     lone ☺ in the (Telegram) test chat is large through the AccessKit tree. What each app does, read from their
     public source (behaviour only, nothing copied) and not tried by hand:
     - **Signal:** Android draws a text-default symbol as its own emoji with or without U+FE0F (its emoji tree
       treats a trailing U+FE0F as optional; `lib/emoji/.../parsing/EmojiTree.java`) and ignores U+FE0E; whether
       its downloaded emoji data has © ® ™ was not checked. Desktop is strict: only fully qualified sequences are
       keys of its emoji data (`ts/axo/emoji.std.ts`, `scripts/generate-emoji-data.mjs`), so a bare ☺ stays text
       there. iOS leaves it to Apple's renderer (not checked). PingCraft follows the phones, as the report did, so
       it differs from Signal Desktop: **for the maintainer to confirm.**
       Signal's large messages are up to 5 emoji (`JumboEmoji.kt`, `kMaxJumbomojiCount`).
     - **Telegram:** Desktop treats U+FE0F as optional except for exactly ™ © ® (`desktop-app/codegen`
       `codegen/emoji/data.cpp`, `generator.cpp`; `lib_ui` `ui/text/text_block_parser.cpp`), so bare ☺ ❤ ✔ ↔ are
       emoji; no handling of U+FE0E was found. Android makes everything in its emoji list an emoji, © ® ™ included,
       and swallows U+FE0E (`Emoji.java`, `EmojiData.java`). PingCraft follows Desktop. Large: up to 3
       (`kIsolatedEmojiLimit`).
     - **WhatsApp:** closed source; no public description was found. From memory of its Web and Android apps a bare
       ❤ is a red heart. **Unsure**: chosen like Signal's phones for the maintainer to confirm.
     - **Discord:** not researched beyond user reports of ™ being turned into an emoji (Discord's support forum,
       "Don't correct trademark symbol to emoji"), which suggests it is lenient. **Unsure**, so `Unicode` until its
       ticket (corrected on 2026-11-26: the first commit had it like Signal).
     - **Element (Matrix):** Element Web marks up only `\p{RGI_Emoji}` (fully qualified) and explicitly not before
       U+FE0E (`apps/web/src/HtmlUtils.tsx`); what a bare ☺ looks like then depends on the font stack (Inter, then
       the system's colour emoji font). Element X iOS's "only emoji" check (matrix-rust-sdk
       `contains_only_emojis`) accepts unqualified forms through the `emojis` crate. Matrix stays `Unicode`.
     Sources at the default branches of signalapp/Signal-Desktop (01e5971), signalapp/Signal-Android (2dd5703),
     signalapp/Signal-iOS (4537228), desktop-app/codegen (0696d38), desktop-app/lib_ui (b635328), DrKLO/Telegram
     (21f7d10), element-hq/element-web (4a1cfaf), element-hq/element-x-ios (476d869), matrix-org/matrix-rust-sdk
     (7dabadc), read on 2026-11-26.
2. **Done (2026-11-25), with gaps.** **Reactions.** Show reactions under a message with counts and who reacted; add and remove one from the picker or a
   quick row of recent ones.
   Built: a chip per reaction under the message (the count, who reacted on hover and in its accessible name, in the
   accent when you are one of them; clicking it adds or takes back yours), and six quick reactions in the message's
   menu (👍 ❤️ 😂 😮 😢 🙏, sent with the variation selector; a test checks each has a glyph). With ticket 1:
   recently used emoji come first in the quick row, and MORE REACTIONS… opens the emoji picker. Open: what each
   bridge carries (pending: needs the maintainer's accounts).
3. **Done (2026-11-25), with gaps.** **Replies.** Reply to a message: the reply shows a quoted line of the original, and clicking it scrolls to it
   (loading older messages if needed).
   Built: REPLY in the message's menu, a "Replying to …" bar over the composer, the quote above a reply (an
   original that is not loaded is fetched from the server). Clicking the quote scrolls to the original; when it is
   older than what is loaded, it loads one page of older messages, and a second click goes on. Open: loading until
   the original is found in one click; the per-bridge check (pending).
4. **Done (2026-11-25), except the earlier versions.** **Editing.** Edit your own messages; edited messages say "edited", and the earlier versions can be seen in
   message details.
   Built: EDIT in your own text message's menu puts its text in the composer (the draft waits and comes back), SAVE
   sends the edit, and edited messages say "edited". Earlier versions come with ticket 6. Per-bridge check pending.
5. **Done (2026-11-25), with the per-bridge check pending.** **Deleting.** Delete your own messages (for everyone, where the network allows it), with a confirmation; a
   deleted message leaves a "message deleted" line. What happens on the other network is checked per bridge.
   Built: DELETE in your own message's menu asks first, then redacts it; any deleted message shows "Message
   deleted". The sheet says whether it disappears on the network depends on the network.
6. **Done (2026-11-25), with gaps.** **Message details.** Sent and received times, delivery and read receipts where the network reports them, which
   network and account it came through, encryption state, and edit history.
   Built: DETAILS in the message's menu opens a sheet with when it was sent, the network (and that it came through
   your bridge), its encryption in the header's words, who has read it and when (from read receipts, which the
   timeline now tracks), and the earlier versions of an edited message (also ticket 4's gap). Open: a separate
   "received" time (Matrix keeps one timestamp; a bridged message carries the network's), delivery receipts, and the
   account line (`Chat.account` is still `None`, milestone 2 ticket 6). Per-bridge check of receipts pending.
7. **Done (2026-11-25).** **Selecting and copying.** Select text inside a message and copy it; copy a whole message; select several
   messages to copy, forward or delete together.
   Built: message text is selectable (egui's selectable labels; not tested by dragging); COPY TEXT in the menu;
   SELECT starts selecting, with a checkbox on each message and a bar instead of the composer: COPY ("sender,
   time: text" per line), FORWARD, DELETE (only when all are yours; asks once) and CANCEL.
8. **Done (2026-11-25), with gaps.** **Forwarding.** Forward one or more messages to another chat, on any connected network, with a note that it was
   forwarded where the network supports that.
   Built: FORWARD in the menu or the selection bar opens a sheet to search and pick one chat; the text is sent there
   as new messages from you, through that chat's send queue. The sheet says they don't say "forwarded": no bridge
   carries that, so nothing marks them yet. Open: forwarding media (with milestone 6's media), several target
   chats at once, and a "forwarded" mark if bridges gain one.
9. **Done (2026-11-25), with gaps.** **Pinning.** Pin messages inside a chat (shown at the top of the chat, synced to the network where the bridge
   carries pins), and pin chats to the top of the inbox.
   Built: PIN IN THIS CHAT / UNPIN in the message's menu (`m.room.pinned_events`, so everyone in the chat sees
   them; refused without permission, said in plain words), and a pinned bar under the chat header, newest pin first,
   that goes to the message when clicked and steps through the pins. The chat's "…" menu pins it to the top of the
   inbox (the `m.favourite` tag, so other Matrix apps see it; confirmed by the maintainer, 2026-11-25); pinned
   chats come first, with a pin. Open: a pinned
   message that is not loaded shows without its text until older messages are loaded; whether bridges carry pins
   (pending, needs the maintainer's accounts).
10. **Done for chats (2026-11-25), with gaps.** **Muting.** Mute a chat or a whole network, for an hour, a day, until turned back on, or until a time; muted
    chats still count as unread but make no notification. Possibly mentions-only for group chats.
    Built: the chat's "…" menu mutes it for an hour, a day, a week or until turned back on, and turns it back on.
    Muting is a push rule on the server, so the user's other Matrix apps follow it; the end time is kept in the
    chat's room account data (`io.github.jonasbove.pingcraft.mute_until`) and PingCraft turns notifications back on at that time
    while it runs (a push rule has no end time; if PingCraft is not running, it happens at its next start). Muted
    chats show a struck bell and a quiet count, and still count as unread. A mute or unmute made on another device
    (2026-11-26) shows as soon as the sync brings its push rule: the room list does not watch `m.push_rules`, so the
    inbox wakes on it and sends the chats whose rule now says otherwise than what it last sent. The end time is saved
    before the rule both ways, so it can arrive first: on its own it sends a chat only while the rule says muted with
    an end, so a mute never shows as off before its rule arrives, nor an unmute as "until turned back on" (unit
    tests; a live test with one user on two devices, `tests/inbox.rs`, a_mute_made_on_another_device_shows_in_the_inbox,
    not yet run against a server). Open: a chat whose mute's end time changes on another device while it stays muted
    for good (from an end to none) is not sent again until its next change; PingCraft's menu cannot do that. Open: muting a whole network, "until a
    time" (needs a date and time picker), mentions-only. PingCraft's own desktop notifications (milestone 6,
    ticket 1) leave muted chats out.
11. **Notification sounds and colours.** Choose a sound for each network or chat (a few built-in sounds under an
    allowlisted licence, or none), and pick a chat's or network's accent colour, tied to the network colours under
    "Later". Builds on milestone 6's desktop notifications.
    **First step done (2026-11-26), with gaps** (maintainer, 2026-11-26: system sound first). Per chat and per
    network: the notification sound on or off, and an accent colour.
    - **Sound:** the operating system's own notification sound, no audio library and no sound files. Linux and the
      BSDs: the freedesktop hint `sound-name = message-new-instant` (the user's sound theme), or `suppress-sound`
      when off (tested: the hints sent). Windows: the toast's `Default` sound when on; without it a toast was and is
      silent (built with `cargo check --target aarch64-pc-windows-msvc`, not run). **macOS: not done** — notify-rust
      4.19 cannot ask for `UNNotificationSound.defaultSound`, so notifications there stay silent whatever the
      setting. **Default: on** (decided here, for the maintainer to confirm): that is a change on Windows, where
      toasts were silent, and on Linux services that play no sound unless asked.
    - **Accent colour:** any colour (maintainer, 2026-11-26), saved as `#rrggbb`; it colours the chat's network tag
      in the inbox and the chat header. A network's own colour (`Accent::of_network`, the shade drawn in a light
      theme) is drawn in the theme's shade of it, so it stays legible in a dark theme. Data saved before then holds a
      network's id (`"signal"`): it reads as that network's colour (unit tests for both kinds of data) and is written
      as hex the next time. A network's colour is set on the Settings screen with craft-color-ui's colour picker or
      as hex; a chat's own in its "…" menu, still as swatches of the four network colours.
    - **Where they live** (decided here, for the maintainer to confirm): with the account, so they follow the user
      between devices, in PingCraft's own namespace, since no Matrix event says how a client sounds or colours a
      chat. A chat's: room account data `io.github.jonasbove.pingcraft.alerts` (`{"sound": false, "accent":
      "signal"}`, a key left out follows the network), read with the inbox like `mute_until`. A network's: global
      account data `io.github.jonasbove.pingcraft.networks` (one entry per network), changed on top of the server's
      copy (other networks and keys kept), watched for other devices' changes. A chat's setting wins over its
      network's, which wins over the default. They are PingCraft's only; the networks are not told, as the menus say.
    - **Where they are set:** a SOUND AND COLOUR part of the chat's "…" menu (with USE THE NETWORK'S SOUND SETTING
      once the chat has its own), and each network's on the Settings screen's Notifications page (Matrix too; moved
      there from the ACCOUNTS tab on 2026-11-26, for the maintainer to confirm). **No frames yet** for either.
    - Tests: the levels and the colour ids (core), reading and writing both kinds of data and refusing anything else
      (adapter), the Linux hints, and through the AccessKit tree the chat menu's controls, a network's setting
      reaching a chat, a notification's sound following the chat over its network, and the ACCOUNTS tab's controls.
      Live tests (`tests/network_alerts.rs`, two devices; the chat's in `tests/timeline.rs`) **were not run** (no
      test server in this session). Not heard: no sound was played by hand on any system.
    - **Open:** custom sounds (a few built-in sounds under an allowlisted licence) are later; sound per account (two
      Telegram accounts) and the "Later" network colours themselves (`inbox-networks`) are not decided.
12. **Other everyday actions,** to decide when the design is made: mark as unread, archive a chat, drafts kept
    per chat, typing indicators, a "jump to latest" button, and the context menu that holds all of the above
    (right-click and a keyboard shortcut, through `craft-libs` `menu` and `keymap`).
    - **Mark as unread: done (2026-11-25).** In the chat's "…" menu (`m.marked_unread`); it leaves the chat, the
      row gets an accent mark and counts under UNREAD, and opening the chat takes the mark off.
    - **Drafts kept per chat: done (2026-11-25).** Leaving a chat keeps what was typed in the SDK's composer
      draft, in the encrypted state store (never a plain file, ADR 0004), so it survives a restart; the inbox row
      says "Draft: …" and the text comes back when the chat is opened. **A reply or edit in progress is kept with
      it (2026-11-26)**, as the SDK's draft kind (`ComposerDraftType::Reply`/`Edit` with the message's event ID, so
      it lasts across restarts), and comes back as the reply bar or the edit once the chat's messages have loaded;
      when that message is not among the loaded ones, the reply or edit is dropped and the text stays as a new
      message's draft. The SDK keeps one draft per chat, so during an edit only one thing can be kept (decided here,
      for the maintainer to confirm): an edit that changed the text is kept, and the draft it set aside is lost; an
      edit left unchanged is no work, so the set-aside draft is kept instead. **The open chat's draft is saved when
      PingCraft closes** (2026-11-26): `on_exit` queues it (only it: not the "stopped typing" notice, a request the closing window would wait on)
      before the engine is dropped, and the engine handles what
      was queued before it stops (tokio's channel hands out queued commands before reporting it closed). Tests: the
      AccessKit tree for keeping and restoring a reply and an edit, the unchanged edit, a reply whose message is not
      loaded, and closing (which only checks that the command is sent); the round trip of the draft kind through the
      SDK's type (unit). The live draft test (`tests/timeline.rs`) now also keeps a reply draft and reads it back; it **was
      not run** (no test server in this session). Not tried by hand.
    - **Typing indicators: done (2026-11-25).** "Lars Holm is typing…" over the composer (two names, or
      "several people"); PingCraft tells the chat while the user types (again every 20 s) and when they send or
      empty the field. Per-bridge check pending.
    - **Jump to latest: done (2026-11-25).** JUMP TO LATEST over the timeline when it is scrolled up.
    - **Archive a chat: done (2026-11-26), without a frame.** ARCHIVE in the chat's "…" menu leaves the chat and
      moves it out of the inbox; an ARCHIVED filter next to ALL and UNREAD lists the archived chats (with its own
      count; ALL and UNREAD leave them out, so they also leave the unread count), and their menu says MOVE BACK TO
      THE INBOX. An archived chat comes back by itself when a message newer than the archiving arrives, unless it is
      muted (then it stays archived whatever arrives). Mechanism (decided here, for the maintainer to confirm): PingCraft's
      own room account data `io.github.jonasbove.pingcraft.archived` with `{"at": <ms>}` (0 when not archived), the
      same way as `mute_until`, not the `m.lowpriority` tag: other Matrix apps show that tag as a "low priority"
      section rather than out of the inbox, the SDK removes the chat's `m.favourite` (its pin) when it sets it, and a
      tag cannot carry the time the chat was archived. "Comes back" is worked out from that time and the last
      message's (`Chat::is_archived`), so nothing has to be written when a message arrives and it works the same at
      start-up, after a restart and when PingCraft was not running; the time is now or the last message's time if
      that is later, so a sender's clock running ahead does not bring it straight back. Other Matrix apps do not see
      it as archived. Tests: the rule (core unit tests: counts, muted, no messages, coming back), the archive time,
      and the menu, the filter and coming back through the AccessKit tree. A live check (in `tests/timeline.rs`, the
      chat settings test) archives a chat and waits for the inbox to show it archived; it **was not run** (no test
      server in this session). Another device's archiving reaches the inbox as soon as the sync brings it (the inbox
      watches the sync's room account data, 2026-11-26), but no live test has two devices for it yet; not tried by
      hand. **Needs a frame** on page "Messages · Draft" (the filter and the
      menu items), not drawn in this change.
    - **On craft-ui's menu, with letter keys (craft-libs update, part 1, 2026-11-26).** The message's menu is
      craft-ui's `Menu`, opened from its "more" button or a right-click on the message or its text; rows a message
      does not allow are left out, as before. The quick reactions are rows too ("React with 👍", checked when the
      reaction is yours; their emoji come from the font stack's colour face): as egui buttons in the menu's body they
      kept the keyboard out of the whole menu. Each row's label marks the design's letter as its mnemonic, which
      craft-ui's open menu runs it from, and shows it as the row's shortcut: R reply, E more reactions, C copy text,
      F forward, P pin, S select, I details, and T edit and D delete (the design's Up moves among the rows, and Delete
      is no letter). Ctrl+C is not bound. Tests: Enter opens the menu, the arrows and Enter run a row, Escape closes
      it, R starts a reply, and r typed in the composer is text. Not tried in the running app. Not done: the chat's
      "…" menu (still an egui popup) and the app menu (craft-menu). (The note above that craft-libs `menu` is only
      the window's menu bar is out of date: craft-menu has context menus, `craft_menu::context`.)
    - **Context menu: done, without shortcuts.** A message's menu opens from its "…" button or a right-click;
      the keyboard shortcuts in the design (through `craft-keymap`) are still to come. **They come with the
      craft-libs update** (maintainer, 2026-11-26): the bump from `cdee9b8` (done 2026-11-26, now `a6103343`) brings craft-keymap's and
      craft-menu's shortcuts, so they are not built on the old pin. The draft frames' keys, for then: R reply,
      E react, Ctrl+C copy text, F forward, P pin, S select, I details, Up edit, Del delete (while the menu is open;
      never while a text field has focus).
13. **PingCraft's own interface icons** (maintainer, 2026-11-25). A set of a few dozen original icons, drawn for
    PingCraft as SVGs in the design's language (hairline strokes, square ends): reply, forward, react, more, pin,
    unpin, mute and unmute, edit, delete, select, copy, details, jump to latest, search, the encryption marks, and
    a mark for each network that is our own drawing, never a network's logo (clean room). Designed in FigCraft first
    (page "Components", where the draft frames' icons are placeholders), then drawn in egui in place of the text
    marks used today (📌, 🔕, "…", "×", "›"). Each recorded in `ATTRIBUTION.md`; checked with craft-libs `ui`
    first (house rule 16).
    **Superseded by craft-ui's icons** (maintainer, 2026-11-26: craft-libs dictates the look). Done (craft-libs
    update, part 1): the marks are craft-ui's catalogue icons (`craft_ui` icons, Lucide by default): the message's
    and the chat's menus open from `IconButton`s with `more`, the next pinned message is `chevron_right`, cancelling
    a reply or edit is `close`, and a chat row's pinned and muted marks are labelled `Icon`s ("Pinned", "Muted";
    they were bare emoji before, which a screen reader read as such). PingCraft draws no icons of its own, so
    nothing goes in `ATTRIBUTION.md`. craft-ui's runtime is installed for this (`framework.rs`, the `embed` hooks in
    the desktop app; the tests install it headless). Still text, with no catalogue icon to take: the composer's
    emoji button (an emoji, 😀) and a loading message's "…". The menus' rows stay words; icons for reply, forward,
    react, select and the rest come with the menus (part 1, step 4) where the catalogue has them. Network marks:
    the network tags (craft-ui badges, step 3); no network logos, as before.

## craft-libs update, part 1: PingCraft on craft-ui (2026-11-26)

The maintainer's rule (2026-11-26): craft-libs is the standard and dictates how PingCraft looks and feels. Part 1
(branch `craft-ui-1`) moves the interface onto craft-ui; the window and title bar, platform appearance, dialogs on
craft-ui's dialog host, the i18n swap, the automation interface and colour themes come later.

- **Fonts** (step 1): craft-ui's font stack, see M2 ticket 9 and "Messages and chats" 1.
- **Runtime and icons** (step 2): see "Messages and chats" 13. The runtime is hosted through `craft_ui::shell::embed`
  (`framework.rs`). **Not run:** the desktop app's hooks (`raw_input_hook`, `logic`, `before_ui`), `SystemWindows` and
  the OS appearance reader only compile; the tests install the runtime headless and run no hooks, and nobody has
  opened the window since.
- **Theme and components** (step 3): PingCraft's own look is gone: the orchid tokens, the square corners, the mono
  capitals and the hand-made buttons, toggles and badges. Colours, sizes and type come from craft-ui's resolved
  theme, its default theme (part 1 said "in the system's light or dark", but `RootEnv::new()` is light; since Settings
  the mode is the user's choice, the system's by default), read through `theme::Tokens` and written
  into egui's style for the parts still drawn with egui (`theme::apply`, each frame, only when they change). Buttons
  are craft-ui `Button`s (primary, secondary, danger), fields `TextField`s (Enter submits through the field's own
  output), check boxes `Checkbox`es, the header's tabs `ToggleButton`s, the ALL / UNREAD / ARCHIVED filter a
  `Segmented`, the unread count a count `Badge`, the encryption state a `Badge`, the risk scale `Badge`s, labels and
  notes `Text` in the caption and body roles, and the avatar a round mark in the accent's soft fill. Labels are in
  sentence case now ("Sign in", not "SIGN IN"); the tests find them so. The sheets are craft-ui `Dialog`s (`Dialog::prompt`,
  `widgets::sheet`): under an egui `Modal`, Tab reached nothing once the sheets' controls were craft-ui components, so
  they moved to craft-ui's own dialog, which keeps Tab inside and closes on Escape or its close action; that is the
  sheet's Cancel wherever it has one (the sign-in, unlock, recovery key, verification, waiting and "already open"
  sheets have none, so Escape leaves them open). Their logo and orchid hero are gone; their footer is PingCraft's
  row of craft-ui buttons, not the dialog's own `FooterButton`s, and running them on craft-ui's `DialogHost` is the
  later "dialogs" work. Tests: Tab stays in the risk sheet and Escape cancels it; Enter in the password signs in and
  Enter in the composer sends. The reaction picker is still an egui `Modal` (its content is the egui emoji picker). Still egui, with a reason each: the
  composer and the inbox search (`TextField` cannot lay its text out with PingCraft's emoji layout), the emoji
  chips, quick reactions and picker cells (a craft-ui button's label is plain text, not a layout job), the message
  list and inbox rows, and the menus' rows (step 4). Network colours stay data (`theme.rs`), a dot beside the
  network's neutral badge.
  - **The accent colour picker** (ticket 11), after part 1: decided by the maintainer (2026-11-26) as any colour,
    picked with craft-color-ui's colour picker and saved as `#rrggbb` in the same account data; done with the
    Settings screen (see "Messages and chats" 11).
  - **Decided (maintainer, 2026-11-26): PingCraft's own theme is dropped.** PingCraft opens in craft-libs' default
    theme; no orchid theme or app section is shipped (M6 ticket 7 updated).
  - **The design file** (`design/`) now differs from the built app everywhere the look changed: corners, type,
    sentence-case labels, buttons, tabs, badges, icons and the sheets' hero (no orchid band or rule). It is to be
    redrawn later.
  - **Not looked at by eye:** no screenshot was taken; the tests check the AccessKit tree and some geometry only.
    Built for Linux and wasm32 only; Windows, macOS and the BSDs were not compiled.
  - **Departures from craft-ui's own patterns:** a field is a caption over a `TextField`, not `form::Field`; the
    header's tabs are `ToggleButton`s, not a `TabStrip`; the chat's "…" menu is still an egui popup.
  - **Decided (maintainer, 2026-11-26): installed system fonts stay on for the desktop.** craft-ui's `system_fonts`
    draws names and messages in scripts the interface faces lack from the system's fonts; it brings `fontique` and
    fontconfig (loaded at run time) into the desktop build. The tests run with it off.
  - **Test assertions changed in part 1, and why:** the theme's contrast test now checks the network colours only, at
    the 3:1 a non-text cue needs (their names are drawn in the theme's text colour; the theme's own contrast is
    craft-theme's job); the check that the pinned and muted marks have glyphs went with the emoji marks (they are
    icons); the risk scale's level is told by its description, not a toggled state (a badge has none); sheet titles
    are found on their dialog node, a footer's Close below the dialog's own; the harness runs up to 12 steps while a
    dialog opens; labels are in sentence case.
- **The corner rule** (maintainer, 2026-11-26): what can be clicked, typed in or chosen (buttons, fields, chips,
  swatches, menu rows, a clickable row's highlight) is rounded with craft-ui's control radius (`Tokens::radius_control`),
  ideally by being craft-ui's own component; surfaces that sit apart from their surroundings (cards, the account rows,
  message bubbles, boxed notes in sheets, the emoji picker) take craft-ui's surface radius (`radius_surface`), as its
  dialogs and menus do; labels, tags, badges and status words may be square, but nothing that is not interactive may
  look like a button. Regions that run edge to edge (panels, the header, the problem and reply bars) stay square,
  and the QR code is square by nature. Done 2026-11-26 across `crates/ui-egui`; "Connected" on a card is a craft-ui
  `Badge` in line with Connect. The rounded surfaces are still egui frames with craft-ui's radius, not craft-ui's own
  surface (`Stack::surface`), since their content is egui's.
- **craft-libs gaps found with the dialogs and menus:** a `Dialog`'s close action cannot be turned off (its
  `closable` has no setter), so the sheets that have nothing to cancel show a close action that does nothing; egui's
  `Popup::menu` does not see the framework's click (`widgets::menu_under` opens it from the button's output); a menu
  row's label is plain text (emoji draw through the font stack's fallback, not PingCraft's emoji layout).
- **Keyboard shortcuts and menus** (step 4): the message menu is craft-ui's `Menu` with the design's letters.
  **PingCraft's commands** (`commands.rs`, rule 14) are registered with craft-ui's command board as an app catalog
  (`AppCatalog`): the tabs (`view.inbox`, `view.accounts`, `app.settings`), `view.refresh`, the three appearance modes
  and Settings' actions (`security.change_passphrase`, `security.new_recovery_key`, `security.enter_recovery_key`,
  `security.verify_this_device`, `security.sign_out`), each with its enabled state from the app's state. Only
  Settings has a default shortcut, ⌘, (Ctrl+, off macOS), as in the other Craft apps; the others wait for the design.
  A shortcut, a menu row and a command button (`Button::command`) all fire into the frame's output, which
  `framework::before_input` keeps for the app to run (`PingCraftApp::run`); the header's tabs and the Appearance
  page run the same function. Settings' actions that take a value (previews, a network's sound and colour, the skin
  tone, renaming this device, verifying or signing out another device) are `SettingAction`s: their ids are registered
  too, a fired reference carries the value in its parameters, and the page's controls run the same
  `settings::run_action` (2026-11-26).
  **Shortcuts** (maintainer, 2026-11-26), with `Cmd` ⌘ on macOS and Ctrl elsewhere, `Ctrl` the Control key: ⌘,
  Settings; ⌘1 Inbox and ⌘2 Accounts; ⌘F search chats (the Craft apps' find); F5 and ⌘R Refresh (craft-libs has no
  convention for it; decided here); Ctrl+Tab / Ctrl+⇧Tab, ⌥↓ / ⌥↑ and ⌘PgDn / ⌘PgUp next and previous chat in the
  inbox's order with its filter and search, wrapping; Ctrl+⌥Tab / Ctrl+⌥⇧Tab and ⌥⇧↓ / ⌥⇧↑ next and previous unread
  chat; ⌘⇧A mark the open chat read (new core command `MarkRead`). Opening a chat this way marks it read as a click
  does. Escape in an open chat goes back to the list when no field, menu, popup or dialog takes it (the window's own
  Escape, `with_window_intents`). Menus: Go (Inbox, Accounts, Search chats, the four moves) and Chat (Mark as read);
  View keeps Refresh and Appearance. Ctrl+Tab works from the composer (tested). A desktop may keep Ctrl+⌥Tab for
  itself; not worked around. ⌥ chords do not fire while a text field has the keys (craft-keymap's typing rule).
  Kittests in `tests/shortcuts.rs`. **The app menu** is craft-menu's in-window bar (`MenuBarUi`) over File (Sign out…), Edit
  (Settings, tagged `ItemRole::Settings`, which the Linux and Windows layouts put last in Edit as "Preferences…")
  and View (Inbox, Accounts, Refresh, Appearance). **Not done:** the native macOS menu (`craft_menu::mac::install`,
  with its per-frame poll and gate; macOS is not compiled here), so a Mac build would show the in-window bar; the
  chat's "…" menu stays an egui popup (it holds the chat's sound check box and colour swatches, which are not menu
  rows, so it is not a small move); the Keyboard Shortcuts dialog (`craft_menu::shortcuts`) and stored overrides.
  Tests: Ctrl+, and Edit, Preferences… open Settings (AccessKit tree).
- **The Settings screen** (maintainer, 2026-11-26, step 5): **built**, on craft-ui-patterns' `PreferencesPage` in its
  default layout (a search field, the page list, the shown page; each edit applies at once), the SETTINGS tab enabled.
  See M3 ticket 11 for the pages. Tests: `tests/settings.rs` (AccessKit tree, one test per page and per Security
  action, Tab and Enter reaching a page's button), unit tests of the stored formats, and live tests in
  `crates/matrix/tests/settings.rs` (**not run** in this session: no test server).

## craft-libs: what PingCraft lacks

The maintainer's rule (2026-11-26): craft-libs dictates how PingCraft looks and feels. PingCraft adopts craft-ui as
it is, never changes craft-libs to suit itself, and where something it needs is missing keeps a small stopgap built
from craft-libs' own pieces and writes the gap down here, for a craft-libs change someone else makes.

- **Colour emoji that lead a family, and a served emoji face fetched at start** (`craft_ui::fonts`). The stack puts
  `Emoji` faces after the system fonts, as a fallback: a keycap (`1️⃣`) or `❤️` starts in the text face, because
  egui picks a face by a cluster's first character and Inter has the digits. Stopgap: `emoji::stack` names an
  `emoji` family (`UiFonts::family`) and puts the colour face at its head with `Stack::insert`. On the web,
  `Emoji::served` is fetched only when no installed face has a character, and `Emoji::monochrome` always has it
  first (`FaceList` answers before the loader), so a served colour face behind a monochrome one is never fetched;
  and a served face cannot be put at a family's head before it arrives. Stopgap: PingCraft fetches the pinned file
  itself (`emoji::web`, as before) and installs the stack again with it as `Emoji::color`. Missing in craft-libs: a
  way to fetch a served face at install (as `ui_downloads` does for the interface language's scripts) and to name a
  served face as a family's first face.
- **A browser `FontFetcher`.** craft-ui defines the trait but ships no implementation; vectorcraft-web carries its
  own (`BrowserFetcher`). PingCraft does not need one while the stopgap above stands.
- **Icons the catalogue lacks** (`craft_ui` icons at the pinned revision): notifications off (a bell with a slash;
  `track_mute`, a muted speaker, stands in for a muted chat), an emoji or smiley face (the composer's emoji button
  stays the emoji 😀), reply, forward, react (add a reaction), unpin, select, send, archive and mark as unread.
  Present and used where they fit: `more`, `close`, `chevron_right`, `pin`, `copy`, `trash`, `pencil`, `info`,
  `arrow_down`, `lock`, `search`, `settings`.
- **A theme for an app's own egui code** (`craft_ui::style`). craft-framework.md says `craft_ui::style` turns tokens
  into egui styling "stock widgets included", but at the pinned revision nothing public does: the resolved tokens are
  `Env::tokens` (doc-hidden, "read in `style` only"), and the other apps keep their own `theme.rs`. Stopgap:
  `theme::Tokens` reads them through `Env::of_root` and `theme::apply` writes egui's visuals and text sizes.
- **A badge in a colour from data** (`craft_ui::display::Badge`). It takes a tone only. Stopgap: a dot in the
  network's colour beside a neutral badge (`widgets::network_tag_in`).
- **A badge's selected state** (`Badge`). `Props::selected` does not reach its AccessKit node. Stopgap: the risk
  scale's level badge carries the description "This network's level".
- **Rich labels** (`Button`, `TextField`, `Text`): no way to give a label or a field's text a laid-out job, so emoji
  in buttons, chips and the composer cannot take PingCraft's emoji layout. Those stay egui widgets.
- **Emoji layout** (`emoji.rs`): craft-ui's text exports no equivalent (its cluster script rules,
  `fonts/script.rs`, are crate-private) of PingCraft's emoji a size up among text, large
  emoji-only messages, or a network's own emoji presentation (`pingcraft_core::EmojiPresentation`), so these stay
  PingCraft's.

- **A surface component for an app's own egui content.** Cards, account rows, bubbles and boxed notes are egui
  frames given craft-ui's surface radius by hand (`Tokens::radius_surface`), not craft-ui's surface
  (`Stack::surface`), because what they hold is egui's; likewise the chat menu's colour swatches are painted with the
  control radius rather than craft-ui's `Swatch`, which reports a colour pick rather than a choice among fixed
  colours. The PingCraft-specific heights (header 48, footer 34, panels) have no craft-ui token: they are layout,
  not controls.
- **Reset Section on every page** (`patterns::preferences::PreferencesPage`). It shows on every page with rows, and a
  page cannot leave it out; on Settings' Security page nothing has a default, so it does nothing there. Missing: a
  page that opts out of Reset Section.
- **A row's accessible name is its label alone** (`PreferencesPage`). The same row in each network's group reads
  "Notification sound" and "Accent colour"; the group (named by the network) is its context, which a screen reader
  says on entering it. Noted, not worked around.

## Questions for the maintainer (Settings, 2026-11-26)

Decided here the conservative way, each easy to change:
- **Message previews** — answered (maintainer, 2026-11-26): kept with the account, in PingCraft's global account data
  `io.github.jonasbove.pingcraft.notifications` (`{"previews": false}`), asked of the server once signed in and
  followed through the sync. `config.toml`'s `[notifications] previews` stays as this computer's copy and start-up
  value; an "off" there moves up once to an account that keeps nothing (tested; live test `tests/previews.rs`).
- **Appearance** — answered (maintainer, 2026-11-26): stays PingCraft's own, on this computer, for now; it moves to
  craft-theme's suite-wide choice (`selection.json`) in part 2, with the colour themes (M6 ticket 7).
- **Each network's sound and colour moved** from the ACCOUNTS tab to Settings (the ACCOUNTS tab points there). Keep a
  copy on ACCOUNTS?
- **A new recovery key** — answered (maintainer, 2026-11-26): a device that cannot open the backup offers "Reset: make
  a new recovery key…", with a warning and the account password (matrix-rust-sdk's `Recovery::reset_identity`, then
  `enable`). A first key on an account without cross-signing sets cross-signing up first (a live run found that a
  key made without it left the device unable to use it: the SDK counts recovery as on only when the key holds the
  cross-signing secrets), and a key is reported only once the device can use it.
- **Signing another device out** asks for the account password; a server that wants something else (single sign-on)
  gets "PingCraft cannot do that yet".
- **Shortcuts:** only Settings has one (⌘, / Ctrl+,). The tabs, Refresh and the rest have none until the design
  gives them keys.

## Later, not yet planned

- **Network colours and the network filter in the inbox** (maintainer, 2026-11-24; in the design as
  `inbox-networks`, not built). Each network's tag gets its own colour, and a row of toggles hides or shows each
  network's chats. Open questions before it becomes tickets, to grill the maintainer on:
  - whether hiding a network is remembered per device, per user, or only for the session;
  - whether hidden networks still count towards unread totals and notifications;
  - one toggle per network, or per account (two Telegram accounts);
  - how the toggles behave with many networks, and with colour-blind users (the names stay, so colour is never the
    only cue);
  - **workspaces** (maintainer, 2026-11-25): instead of, or on top of, the toggles, named views such as
    "Work" and "Private", each a saved set of networks, accounts or single chats, switched in one click. Open:
    whether a chat can be in more than one, whether they sync across devices (Matrix account data would allow it),
    and whether notifications follow the current workspace.

- **Chat history** (maintainer, 2026-11-25: not for now). PingCraft copies no earlier messages: a chat appears when
  a message arrives in it, with messages from then on. An opt-in asked at each connection was wanted, but a spike
  (bridge images v0.2611.0 on mautrix-go v0.33.0, tuwunel v2.0.0) found it cannot be done cleanly today:
  - **One setting per bridge.** mautrix's history import (backfill) is configured for the whole bridge. There is
    no per-login switch, and the provisioning API's login takes no options.
  - **Only when a chat is created.** tuwunel accepts a bridge's messages with their original times (the
    appservice `ts` parameter), but has no batch sending, so history can only be added while a chat's room is
    first created, never later to a room that already has messages. The choice could not be changed afterwards.
  - **At linking, for WhatsApp and Signal.** WhatsApp sends its history once, when the phone is linked (from the
    code; not tested), and Signal only offers to transfer history if the bridge asks when linking. So the choice
    would have to be made then, by the whole bridge.
  - **Every workaround costs too much for now:** two copies of each bridge (with and without history) doubles the
    containers and shows a contact twice if an account is linked to both; patching the bridges for a per-login
    flag means running modified AGPL bridges (against ADR 0002); turning history on for everyone and hiding it in
    PingCraft still copies it onto the server.

  Worth revisiting if mautrix gains a per-login backfill option, or tuwunel gains batch sending. A Signal-only first
  step is possible (turn backfill on for Signal and let the person accept or decline the transfer on the phone),
  if the phone is confirmed to offer that choice.

- **MailCraft integration** (raised by the maintainer, 2026-11-24). What it means is still open: for example,
  mail as one more network in the inbox, or PingCraft chats shown in MailCraft. It needs its own ADR before any
  ticket.

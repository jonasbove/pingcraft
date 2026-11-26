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
    The header's INBOX and ACCOUNTS tabs (SETTINGS shows, disabled, until it has a screen); the network cards from
    `connect-network`, with each card's accounts and whether its bridge answers; the user's accounts with their state
    and REMOVE, which asks first; and "Connect a network" from the empty inbox. Not checked against a real account
    (tickets 7 to 9), and the screens were checked through their AccessKit tree only, not by eye.
11. **Settings screen** (maintainer, 2026-11-25): the header's SETTINGS tab, disabled until now. Not in the design
    yet: add a `settings` frame to `design/pingcraft.figcraft` first. Sections:
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
   out comes with the Settings screen (milestone 3, ticket 11).

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
   - The web build shows none (the browser's Notification API later); the preview setting comes with Settings.
2. Search.
3. Media: images and files, over authenticated media.
4. Reactions, edits and replies: now part of "Messages and chats" below.
5. Inbox filters by network and account.
6. **Unread count on the app's icon** (maintainer, 2026-11-25): the total unread on the taskbar or dock
   icon, or in the tray where there is one, cleared as chats are read. Per platform: what each desktop supports
   (Linux launchers vary) is checked in the ticket.

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
     starts with egui's monochrome emoji, and `theme::install` starts fetching `noto-color-emoji/Noto-COLRv1.ttf`
     next to the app (through `web-sys`, no JavaScript) without holding up the first frame; once it arrives,
     `emoji::install_color_font` checks it is the pinned file (size, then SHA-256, from `font_pin.rs`) and switches
     faces. A failed fetch or any other file leaves emoji monochrome and says nothing. Tested natively: a context
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
    chats show a struck bell and a quiet count, and still count as unread. Open: muting a whole network, "until a
    time" (needs a date and time picker), mentions-only. PingCraft's own desktop notifications (milestone 6,
    ticket 1) leave muted chats out.
11. **Notification sounds and colours.** Choose a sound for each network or chat (a few built-in sounds under an
    allowlisted licence, or none), and pick a chat's or network's accent colour, tied to the network colours under
    "Later". Builds on milestone 6's desktop notifications.
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
      PingCraft closes** (2026-11-26): `on_exit` queues it before the engine is dropped, and the engine handles what
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
      server in this session). Whether another device's archiving reaches the inbox at once, or only at the next
      change to the chat, is unchecked; not tried by hand. **Needs a frame** on page "Messages · Draft" (the filter and the
      menu items), not drawn in this change.
    - **Context menu: done, without shortcuts.** A message's menu opens from its "…" button or a right-click;
      the keyboard shortcuts in the design (through `craft-keymap`) are still to come.
13. **PingCraft's own interface icons** (maintainer, 2026-11-25). A set of a few dozen original icons, drawn for
    PingCraft as SVGs in the design's language (hairline strokes, square ends): reply, forward, react, more, pin,
    unpin, mute and unmute, edit, delete, select, copy, details, jump to latest, search, the encryption marks, and
    a mark for each network that is our own drawing, never a network's logo (clean room). Designed in FigCraft first
    (page "Components", where the draft frames' icons are placeholders), then drawn in egui in place of the text
    marks used today (📌, 🔕, "…", "×", "›"). Each recorded in `ATTRIBUTION.md`; checked with craft-libs `ui`
    first (house rule 16).

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

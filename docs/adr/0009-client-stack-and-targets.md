# ADR 0009: Client crates, layers and build targets

- **Status:** accepted by the maintainer (2026-11-24)

## Context

The client is Rust and egui on `matrix-sdk` and `matrix-sdk-ui` (0.19.1, 2026-09-18, Apache-2.0). The UI crate's
`RoomListService` and `SyncService` need Simplified Sliding Sync (MSC4186); its `Timeline` also works with
classic `/v3/sync`. Upstream tests `matrix-sdk` on `wasm32-unknown-unknown` (with `js`, `indexeddb` and
`e2e-encryption`), but only compile-checks `matrix-sdk-ui` there. Resolving the dependency tree found no GPL,
AGPL or LGPL-only crates; several are MPL-2.0 (allowed unmodified), and `imbl` declares `MPL-2.0+`.

The other Craft apps keep egui out of everything below the UI crate and check that with `cargo xtask layers`.
CraftCloud releases its desktop app for Linux, Windows, macOS and FreeBSD, plus a web build and a Nix flake.

## Decision

1. **Crates and layers**:

   | Crate | Holds | May depend on |
   |---|---|---|
   | `pingcraft-core` | the domain model (network, account, chat, inbox, message) and the bridge provider interface (ADR 0003) | neither `matrix-sdk` nor egui |
   | `pingcraft-matrix` | the adapter onto `matrix-sdk` / `matrix-sdk-ui`, and the mautrix bridge provider | `core` |
   | `pingcraft-keychain` | the store key in the OS keychain (ADR 0004); desktop only; a candidate for a craft-libs secrets crate | nothing in the workspace |
   | `pingcraft-ui-egui` | the interface | `core` only; never `matrix-sdk` types |
   | apps: `pingcraft` (desktop) and `pingcraft-web` (Trunk) | — | everything below |

   `cargo xtask layers` enforces this.
2. **Shared libraries first:** `craft-ui`, `craft-settings`, `craft-i18n`, `craft-keymap`, `craft-menu`,
   `craft-dialogs`, `craft-platform`, `craft-window`, `craft-web-host`, `craft-markdown`, `craft-html` and
   `craft-mime` from `storytold/craft-libs`, pinned at one revision. Keychain access is local until craft-libs
   has a secrets crate (ADR 0004).
3. **Build targets:** the same as CraftCloud:
   - Linux x86_64 and aarch64 (AppImage, deb, rpm, tar.gz, Flatpak, Arch package) and riscv64 (deb, rpm, tar.gz);
   - Windows x64 and arm64 (msi and portable zip);
   - macOS universal (dmg);
   - FreeBSD x86_64 and aarch64 (tar.gz);
   - the web build for `wasm32-unknown-unknown`, and a Nix flake.

   Every target is built in CI from milestone 2 on, using `storytold/craft-common-actions`. Publishing a release
   is a maintainer decision (house rule 9).
4. **Quality gates:** `cargo xtask ci` runs:
   - fmt;
   - clippy with `-D warnings`;
   - tests;
   - layers;
   - a wasm check of everything except the desktop app;
   - `cargo deny`, with CraftCloud's licence configuration plus `MPL-2.0+` treated as `MPL-2.0`.

   Every crate root carries the never-crash `deny` attribute. Every UI change gets `egui_kittest` tests that
   check the AccessKit tree.

## Alternatives

- **The UI uses SDK types directly:** a faster start, but UI tests then need a server, and every SDK upgrade
  reaches the interface.
- **Desktop only at first:** fewer CI jobs, but a target that does not build gets harder to fix the longer it is
  left.

## Consequences

- Mapping code between `matrix-sdk-ui` and `core` is the price of a testable UI.
- Two risks are spiked in milestone 2:
  - `matrix-sdk` 0.19.1's only TLS backend is `aws-lc`, which needs a C toolchain. It may not build on FreeBSD,
    riscv64 or Windows arm64.
  - `keyring`'s default stores cover macOS, Windows and the Linux Secret Service. FreeBSD and Flatpak need
    checking.
- `matrix-sdk-ui` on wasm is compile-checked upstream, not tested, so the web build has its own spike in
  milestone 4.

## Addendum (2026-11-24): the milestone 2 targets spike

`cargo check` of `pingcraft-matrix` (`matrix-sdk` and `matrix-sdk-ui` 0.19.1 with SQLite, encryption and
`rustls-aws-lc-rs`) and of `pingcraft-keychain` (`keyring` 4.2.0), on Linux x86_64 with Rust 1.100.0. A check
compiles the C parts (`aws-lc-sys`, SQLite) for the target, but links nothing and runs nothing.

| Target | `matrix-sdk` with aws-lc | `keyring` | How |
|---|---|---|---|
| x86_64-unknown-linux-gnu | builds | builds | native |
| aarch64-unknown-linux-gnu | builds | not run | zig 0.16 as the C cross-compiler |
| riscv64gc-unknown-linux-gnu | builds | not run | zig 0.16 |
| x86_64-unknown-freebsd | builds | builds | zig 0.16 |
| wasm32-unknown-unknown | builds (`js`, `indexeddb`, encryption; `matrix-sdk-ui` with `js`) | not used on the web | native |
| Windows x64 and arm64 (MSVC), macOS | not run | not run | needs Microsoft's or Apple's toolchain; CI |

Findings:

- **`aws-lc` is not a blocker on Linux or FreeBSD.** Its C code cross-compiles with an ordinary C toolchain. The
  `cc` crate passes `--target=<rust triple>`, which zig rejects, so a zig wrapper must drop that flag. CI on each
  native runner needs no wrapper.
- **`keyring` 4.2.0's default store on FreeBSD** (and on every Unix except macOS) is the Secret Service over
  D-Bus, the same as on Linux. It never falls back to a plain file: without a running Secret Service it returns an
  error, and that is when PingCraft asks for an unlock passphrase (ADR 0004).
- **Flatpak:** the Secret Service is reached over D-Bus, so the Flatpak manifest needs
  `--talk-name=org.freedesktop.secrets`. Whether `keyring` can use the Secret portal instead was not checked.
  This was not built or run in a Flatpak here.
- **Still open:** the Windows and macOS targets, and linking and running on every target, wait for CI.

## Addendum (2026-11-25): `pingcraft-notify`

A crate added with milestone 6's desktop notifications, in the layering of decision 1:

| Crate | Holds | May depend on | Layer in `cargo xtask layers` |
|---|---|---|---|
| `pingcraft-notify` | desktop notifications, each operating system its own way (zbus on Linux and the BSDs, notify-rust on macOS and Windows), behind `pingcraft_core::notify::Notifier`; desktop only; a candidate for a craft-libs notification crate | `core` only; neither `matrix-sdk` nor egui | 1, beside `pingcraft-matrix` |

Only the desktop app (layer 3) uses it; the interface decides what to notify and sees only the `Notifier` trait.
The macOS and Windows backends stay (maintainer, 2026-11-25).

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

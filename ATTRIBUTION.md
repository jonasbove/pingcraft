# Attribution

Every asset in this repository, where it came from and under what terms (house rule 3).

## Logo

| Files | Source | Licence |
|---|---|---|
| `design/assets/logo/pingcraft-dolphin.png` | Cut out by `design/tools/logo.py` from an engraved dolphin drawing (black and white ink on a flat blue) supplied by the maintainer on 2026-11-24. The drawing was generated with an AI image tool by the maintainer, for PingCraft (maintainer, 2026-11-25). | MIT OR Apache-2.0 (this repository's licence) |
| `design/assets/logo/pingcraft-logo.pcraft`, `pingcraft-logo-*.png`, `pingcraft-logo.webp`, `pingcraft.ico`, `pingcraft.icns` | Made from the cut-out above by `design/tools/logo.py`: the dolphin on PingCraft's orchid tile | MIT OR Apache-2.0 (this repository's licence) |

The app embeds `pingcraft-logo-128.png` (drawn in the window) and `pingcraft-logo-256.png` (the window icon),
and the packages in `packaging/` carry the PNGs, `pingcraft.ico` and `pingcraft.icns` as the app icon.

## Design

| Files | Source | Licence |
|---|---|---|
| `design/pingcraft.figcraft`, `design/exports/` | Made for PingCraft with `design/tools/`. The look (layout, type scale, theme tokens) follows the Craft launcher's, as CraftCloud's design uses it; no launcher or ArtCraft logo, mark or name is used. | MIT OR Apache-2.0 |
| `design/tools/fc.py`, `lib.py`, parts of `components.py` and `screens.py` | Adapted from CraftCloud's `design/tools/` (same owner) | MIT OR Apache-2.0 |

## Fonts

No font files are in this repository (house rule 3).

| Font | Source | Licence |
|---|---|---|
| Noto Color Emoji, COLRv1 build, v2.057 (`Noto-COLRv1.ttf`, unmodified). Copyright 2022 Google Inc. | [googlefonts/noto-emoji at `99701e07`](https://github.com/googlefonts/noto-emoji/tree/99701e07d3d1fe1bdbcdb14964547f0fd592966f/2D/fonts), SHA-256 `b8e25ea68db82f9e4d0aee921f4420be2be39887bd5c893a2ad98710531f9d0c`. Fetched by `cargo xtask fonts` into the git-ignored `.cache/craft-fonts/` and embedded in the desktop app (`crates/ui-egui/font_pin.rs`, `build.rs`); the web build does not carry it. | OFL-1.1; the licence text is in `NOTICE`, which every package carries |

egui's own fonts (Ubuntu Light, Hack, Noto Emoji, the emoji icon font; OFL-1.1, Ubuntu-font-1.0) come with the
`epaint_default_fonts` crate.

The design is built with Inter, Archivo and Geist Mono (SIL OFL 1.1) from
`storytold/craft-fonts`, passed to `design/tools/build.py` with `--fonts`.

## Colour themes

`crates/ui-egui/themes/*.json` are `craft-design-theme` files written for the Craft suite (the same files as the open
storytold/craft-libs pull requests #3701 and #3704 to #3709), mapped onto craft-theme's seeds and pins by hand from
each palette's published colours; no theme code or file from the palettes' projects is copied. PingCraft carries
them until craft-libs ships them as built-ins.

| File | Palette | Palette's licence |
|---|---|---|
| `catppuccin.json` | [Catppuccin](https://github.com/catppuccin/catppuccin) (Mocha, Latte) | MIT |
| `dracula.json` | [Dracula](https://github.com/dracula/dracula-theme) | MIT |
| `gruvbox.json` | [Gruvbox](https://github.com/morhetz/gruvbox) | MIT |
| `nord.json` | [Nord](https://github.com/nordtheme/nord) | MIT |
| `one-dark.json` | [One Dark](https://github.com/atom/atom/tree/master/packages/one-dark-syntax) (Atom) | MIT |
| `solarized-light.json` | [Solarized](https://github.com/altercation/solarized) | MIT |
| `tokyo-night.json` | [Tokyo Night](https://github.com/enkia/tokyo-night-vscode-theme) (Night, Day) | MIT |

## Vendored code

Third-party crates carried in the repository, patched; what changed and why is in `vendor/README.md`.

| Files | Source | Licence |
|---|---|---|
| `vendor/ruma-common-0.20.2/` | The crates.io release ruma-common 0.20.2 ([ruma/ruma](https://github.com/ruma/ruma) at `68994653`, `crates/ruma-common`), with explicit types on three `deserialize` calls. `LICENSE` from the same commit. | MIT |
| `vendor/ruma-events-0.35.2/` | The crates.io release ruma-events 0.35.2 ([ruma/ruma](https://github.com/ruma/ruma) at `7d271623`, `crates/ruma-events`), with an explicit type on one `deserialize` call. `LICENSE` from the same commit. | MIT |

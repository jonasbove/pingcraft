# Attribution

Every asset in this repository, where it came from and under what terms (house rule 3).

## Logo

| Files | Source | Licence |
|---|---|---|
| `design/assets/logo/pingcraft-dolphin.png` | Cut out by `design/tools/logo.py` from an engraved dolphin drawing (black and white ink on a flat blue) supplied by the maintainer on 2026-11-24. **Origin and licence of the drawing: to be confirmed by the maintainer.** | to be confirmed |
| `design/assets/logo/pingcraft-logo.pcraft`, `pingcraft-logo-*.png`, `pingcraft-logo.webp`, `pingcraft.ico`, `pingcraft.icns` | Made from the cut-out above by `design/tools/logo.py`: the dolphin on PingCraft's orchid tile | as the drawing, plus this repository's licence for the tile and composition |

The app embeds `pingcraft-logo-128.png` (drawn in the window) and `pingcraft-logo-256.png` (the window icon),
and the packages in `packaging/` carry the PNGs, `pingcraft.ico` and `pingcraft.icns` as the app icon. The
drawing's licence must be confirmed before any of them is published.

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

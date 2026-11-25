# Proposal: a colour emoji font

- **Status:** decided by the maintainer (2026-11-25) and built: Noto Color Emoji COLRv1, with the flags, and no
  pull request to craft-fonts or the egui fork. Instead `cargo xtask fonts` fetches the pinned file (the URL and
  SHA-256 below) into a git-ignored folder and the desktop build embeds it; the face order is solved in PingCraft
  by laying emoji clusters out in a family of their own; the web build has no colour font for now. See PLAN.md,
  "Messages and chats" 1. The rest of this page is the research as written before the decision.
- **Source:** `PLAN.md`, "Messages and chats", item 1. Checked 2026-11-25.

## The problem

PingCraft enables egui's `monochrome_emoji_fonts`, so emoji are drawn as black outlines. Some are missing
altogether, including some SAS verification symbols, which is why the verification test uses
`allow_missing_glyphs()`.

## What our egui can already do

The storytold egui fork at `6d518fc` no longer uses ab_glyph. It shapes text with HarfRust and reads fonts with
skrifa.

- **Colour glyphs.** epaint has a `color_fonts` feature, off by default, that paints `COLR`, `CBDT` and `sbix`
  glyphs through vello_cpu [1]. It turns on vello_cpu's `png` and `text` features. Those features pull in `png`
  (MIT OR Apache-2.0) and `glifo` (Apache-2.0 OR MIT), both already in our `Cargo.lock`. The feature is not
  gated to native builds, but it has not been built for wasm32 here.
- **Web fallback.** On the web, eframe draws clusters that no installed font has with the browser's canvas [2].
- **Limit: face choice.** egui picks a face by the cluster's *first character*, and the first face in the
  family chain that has it wins [3]. Noto Color Emoji maps `0`–`9`, `#`, `*`, `©` and `®` (for keycaps). So the
  emoji face must come *after* the text face, and in that position `1️⃣`, and `❤️` where the text face has U+2764,
  stay monochrome.
- **Limit: COLRv1.** No fork test exercises a COLRv1 gradient, so COLRv1 rendering is unverified until our own
  test passes.

## Candidates

| Font | Licence (allowlisted?) | Format | Size | Version / upkeep |
|---|---|---|---|---|
| **Noto Color Emoji COLRv1** [4] | OFL-1.1 (yes) | COLRv1 vector | 5.04 MB; 3.04 MB without flags | Unicode 18.0, v2.057, Sep 2026; active |
| Noto Color Emoji CBDT | OFL-1.1 (yes) | bitmaps | 10.7 MB | same; blurry when scaled up |
| Twemoji Mozilla [5] | CC-BY-4.0 art, Apache-2.0 code (yes) | COLRv0 | 1.47 MB | Twemoji 14, 2022 |
| Twitter Color Emoji [6] | CC-BY-4.0 art, MIT code (yes) | SVG-in-OpenType: egui can't draw it | 6.8 MB zip | 15.1, 2024 |
| Twemoji, jdecked [7] | CC-BY-4.0 art, MIT code (yes) | SVG/PNG, no font | – | v17.0.3, Jun 2026 |
| OpenMoji [8] | CC-BY-SA-4.0 (house list yes; PLAN.md names only OFL-1.1 or CC-BY-4.0) | COLRv0/v1, CBDT | 2.48 MB COLRv1 | 17.0.0, Apr 2026 |
| Fluent Emoji [9] | MIT (yes) | SVG/PNG, no font | – | active; no flags (unverified) |

The other routes are not recommended:

- **System emoji** (eframe's `system_fonts`). It adds no bytes, but emoji look different on each OS, a minimal
  Linux install may have no emoji font at all, and the SAS test can't be strict.
- **One image per emoji** (`egui-twemoji` [10], licensed `Unlicense OR MIT OR Apache-2.0`). It requires egui
  `^0.33`, and its emoji sit outside text layout, so they can't be selected or edited.

## Recommendation

Use **Noto Color Emoji COLRv1, unmodified**, through `color_fonts`. Emoji then stay ordinary glyphs, so
selecting, copying, `TextEdit` and AccessKit keep working. The font is under the same licence as the rest of
craft-fonts. It is the newest candidate, stays sharp at any size, and is half the size of the CBDT build.

- **Desktop:** embed the font.
- **Web:** fetch the TTF from the same origin at startup, through `web-sys`, which eframe already uses. Then
  install it with `Context::set_fonts`.
- **egui's monochrome emoji faces:** take them out of the family chains whenever the colour face is present.
  Because the web build starts without the colour face, the browser draws emoji until it arrives.

### Added to storytold/craft-fonts

- `fonts/noto-color-emoji/Noto-COLRv1.ttf`, byte-identical to upstream.
- `fonts/noto-color-emoji/OFL.txt`, copied from upstream `2D/fonts/LICENSE`.
- A new opt-in `fonts/emoji-manifest.txt`. It can't go in `fonts/manifest.txt`, because every app embeds that
  one, and GridCraft's plan rules out colour emoji. The new manifest has one line:

  ```
  Noto Color Emoji | Regular | fonts/noto-color-emoji/Noto-COLRv1.ttf | Zsye | OFL-1.1 | fonts/noto-color-emoji/OFL.txt | b8e25ea68db82f9e4d0aee921f4420be2be39887bd5c893a2ad98710531f9d0c | https://github.com/googlefonts/noto-emoji/tree/99701e07d3d1fe1bdbcdb14964547f0fd592966f/2D/fonts
  ```

- An `ATTRIBUTION.md` row. Author: Google Inc. (the copyright in the font's `name` table). Source: the pinned
  tree above. Licence: OFL-1.1.
- A README table row and a `docs/integration.md` section for the new manifest, covering the order rule above
  and the web guidance.
- Tests for:
  - the checksum, the licence and the family name;
  - a `COLR` version 1 table;
  - outlines for the 64 SAS emoji.

### Added to PingCraft

- The `build.rs` recipe from craft-fonts' integration guide. This is new work, because PingCraft doesn't use
  craft-fonts yet.
- Each font's `OFL.txt` in the packages, and a line in `NOTICE`.
- A direct `epaint` dependency with the `color_fonts` feature.
- The font chain: the text face, then Noto Color Emoji. When `CRAFT_FONTS` is empty, the behaviour stays as it
  is today.
- Emoji data for the picker and for `:name:`: the `emojis` crate [11], licensed
  `(MIT OR Apache-2.0) AND Unicode-3.0`. It brings Unicode 18.0, gemoji shortcodes, skin-tone variants and CLDR
  order, and `deny.toml` already allows Unicode-3.0. Search keywords and translated names can come later from
  CLDR annotations (Unicode-3.0) [12].
- Grapheme clusters: `unicode-segmentation` 1.13.3 (MIT OR Apache-2.0), already in the lock file. ZWJ
  sequences, skin tones and flags are GSUB ligatures in the font. HarfRust should form them; the test below
  checks it.
- egui_kittest acceptance tests:
  - a snapshot of `👨‍👩‍👧 👍🏽 🇩🇰 ❤️`;
  - the verification test without `allow_missing_glyphs()`, built with `CRAFT_FONTS_REQUIRED=1`.

### Size

The font is 5.04 MB raw, 2.86 MB with gzip and 2.40 MB with brotli. The growth of the binary and of the wasm
build was **not measured**, because nothing was built here.

## Decisions for the maintainer

1. Approve a craft-fonts pull request adding `emoji-manifest.txt`.
2. Choose the font: Noto COLRv1 (OFL-1.1) or another. OpenMoji would need CC-BY-SA-4.0 accepted for PingCraft.
3. Choose how the web build gets it: fetch it lazily (recommended), embed it (5 MB more wasm), or show the
   browser's emoji only.
4. Keep the flags, or take the 3.04 MB build without them, where flags show as letters.
5. Whether to ask the egui fork to prefer a colour face for emoji-presentation clusters, using its
   `has_emoji_presentation`. That would fix `❤️` and `1️⃣`.

## Sources

1. https://github.com/storytold/egui/blob/6d518fc799681571963842f95b55a6b68431b15a/crates/epaint/Cargo.toml,
   `crates/epaint/src/text/font_face.rs`
2. https://github.com/storytold/egui/blob/6d518fc799681571963842f95b55a6b68431b15a/crates/eframe/src/web/canvas_glyphs.rs
3. https://github.com/storytold/egui/blob/6d518fc799681571963842f95b55a6b68431b15a/crates/epaint/src/text/family.rs
4. https://github.com/googlefonts/noto-emoji (README, `2D/fonts/`, release "Unicode 18.0")
5. https://github.com/mozilla/twemoji-colr
6. https://github.com/13rac1/twemoji-color-font
7. https://github.com/jdecked/twemoji
8. https://github.com/hfg-gmuend/openmoji/tree/master/font
9. https://github.com/microsoft/fluentui-emoji
10. https://crates.io/crates/egui-twemoji
11. https://crates.io/crates/emojis
12. https://www.unicode.org/license.txt (GitHub reports CLDR's licence as `NOASSERTION`)

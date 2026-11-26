// The colour emoji font PingCraft embeds on the desktop: Noto Color Emoji (COLRv1), unmodified, from Google's
// noto-emoji repository at a pinned commit (docs/proposals/emoji-font.md, ATTRIBUTION.md). Read by this crate's
// build script and by `cargo xtask fonts`, so the pin lives in one place. Change all four together.

/// The font's place under the fonts folder, as in storytold/craft-fonts' `fonts/` layout.
pub const EMOJI_FONT_PATH: &str = "noto-color-emoji/Noto-COLRv1.ttf";
/// Where `cargo xtask fonts` fetches it: a commit, never a branch.
pub const EMOJI_FONT_URL: &str =
    "https://raw.githubusercontent.com/googlefonts/noto-emoji/99701e07d3d1fe1bdbcdb14964547f0fd592966f/2D/fonts/Noto-COLRv1.ttf";
/// The SHA-256 of the file at that commit (v2.057, Unicode 18.0). A file with any other hash is refused.
pub const EMOJI_FONT_SHA256: &str = "b8e25ea68db82f9e4d0aee921f4420be2be39887bd5c893a2ad98710531f9d0c";
/// Its size in bytes, to stop a download that is far too big before hashing it.
pub const EMOJI_FONT_BYTES: u64 = 5_038_236;
/// The fonts folder when `CRAFT_FONTS_DIR` is not set, relative to the workspace root (git-ignored).
pub const DEFAULT_FONTS_DIR: &str = ".cache/craft-fonts";

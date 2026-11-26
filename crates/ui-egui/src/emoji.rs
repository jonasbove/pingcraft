//! Colour emoji: the font, telling an emoji from text, and laying text out so that emoji take the colour face.
//!
//! egui picks a face by the first character of a grapheme cluster, and the first face in the family that has it
//! wins. Noto Color Emoji has the digits, `#`, `*`, `©` and `®` (for keycaps), so it has to come after the text
//! face, where `1️⃣` would start in the text face and `❤️` too when the text face has U+2764. So emoji clusters are
//! laid out in a family of their own, [`family`], whose first face is the colour one, and the rest of the text
//! keeps its own family ([`job`]). The text itself, and so what a screen reader reads, is unchanged.
//!
//! The desktop build embeds the font. The web build starts with egui's monochrome emoji and fetches the font after
//! the page opens (`web`); `install_color_font` switches faces once it is there.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use unicode_segmentation::UnicodeSegmentation;

/// The colour face's name in egui's font definitions.
const COLOR_FACE: &str = "NotoColorEmoji";
/// egui's monochrome emoji faces, left out when the colour face is there.
const MONOCHROME_FACES: [&str; 2] = ["NotoEmoji-Regular", "emoji-icon-font"];

#[allow(dead_code)]
mod pin {
    include!("../font_pin.rs");
}

/// The colour font, embedded in the desktop build when the build found it (`build.rs`).
#[cfg(color_emoji)]
pub(crate) const EMBEDDED: Option<&[u8]> = Some(include_bytes!(env!("PINGCRAFT_EMOJI_FONT")));
#[cfg(not(color_emoji))]
pub(crate) const EMBEDDED: Option<&[u8]> = None;

/// The family emoji clusters are laid out in: the colour face first when there is one, then the text faces.
pub fn family() -> FontFamily {
    FontFamily::Name("emoji".into())
}

/// egui's default fonts, with the embedded colour emoji face in place of the monochrome ones, and [`family`].
pub fn definitions() -> FontDefinitions {
    definitions_with(EMBEDDED.map(FontData::from_static))
}

/// egui's default fonts with its monochrome emoji, and [`family`]: what the web build starts with.
#[cfg(test)]
pub fn monochrome_definitions() -> FontDefinitions {
    definitions_with(None)
}

/// egui's default fonts, with this colour emoji face (if any) in place of the monochrome ones, and [`family`].
fn definitions_with(font: Option<FontData>) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let colour = font.is_some();
    if let Some(font) = font {
        defs.font_data.insert(COLOR_FACE.to_owned(), Arc::new(font));
        for chain in defs.families.values_mut() {
            chain.retain(|face| !MONOCHROME_FACES.contains(&face.as_str()));
            chain.push(COLOR_FACE.to_owned());
        }
    }
    let mut emoji = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if colour {
        emoji.retain(|face| face != COLOR_FACE);
        emoji.insert(0, COLOR_FACE.to_owned());
    }
    defs.families.insert(family(), emoji);
    defs
}

/// Why a colour font that arrived after start was not installed.
#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRefused {
    /// Bigger than the pinned file: not hashed at all.
    TooBig,
    /// Not the pinned file (its SHA-256 differs).
    NotPinned,
}

/// Installs a colour emoji font that arrived after start (the web build fetches it), in place of egui's monochrome
/// emoji, from the next frame on. Only the pinned file is accepted (`font_pin.rs`), the same one the desktop
/// embeds; anything else leaves the fonts as they are.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn install_color_font(ctx: &egui::Context, bytes: Vec<u8>) -> Result<(), FontRefused> {
    use sha2::{Digest, Sha256};
    if !u64::try_from(bytes.len()).is_ok_and(|n| n <= pin::EMOJI_FONT_BYTES) {
        return Err(FontRefused::TooBig);
    }
    if format!("{:x}", Sha256::digest(&bytes)) != pin::EMOJI_FONT_SHA256 {
        return Err(FontRefused::NotPinned);
    }
    ctx.set_fonts(definitions_with(Some(FontData::from_owned(bytes))));
    Ok(())
}

/// Fetching the colour font in the web build, after the page has opened.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    /// Where the page finds the font: next to the app, at the same place under the folder as in the fonts folder
    /// (Trunk copies the pinned file there).
    pub const FONT_URL: &str = super::pin::EMOJI_FONT_PATH;

    /// Starts fetching the colour font in the background; emoji switch to it once it has arrived and checked out.
    /// The first frames do not wait for it. If it cannot be fetched, or is not the pinned file, emoji stay
    /// monochrome and nothing is said: the app works the same without it.
    pub fn fetch_color_font(ctx: &egui::Context) {
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(bytes) = fetch(FONT_URL).await
                && super::install_color_font(&ctx, bytes).is_ok()
            {
                ctx.request_repaint();
            }
        });
    }

    async fn fetch(url: &str) -> Option<Vec<u8>> {
        let window = web_sys::window()?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url)).await.ok()?.dyn_into().ok()?;
        if !response.ok() {
            return None;
        }
        let buffer = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
        let array = js_sys::Uint8Array::new(&buffer);
        // Not copied at all when it is bigger than the pinned file.
        (u64::from(array.length()) <= super::pin::EMOJI_FONT_BYTES).then(|| array.to_vec())
    }
}

/// Is this grapheme cluster an emoji to draw in colour? Digits, `#`, `*`, `©`, `®` and `❤` on their own are text;
/// with the emoji variation selector (`❤️`) or as a keycap (`1️⃣`) they are emoji.
pub fn is_emoji(cluster: &str) -> bool {
    if cluster.is_ascii() || cluster.contains('\u{fe0e}') {
        return false;
    }
    if cluster.contains(['\u{fe0f}', '\u{20e3}']) {
        return true;
    }
    match emojis::get(cluster) {
        // An emoji that is shown as text unless asked otherwise comes back fully qualified, with the selector.
        Some(emoji) => emoji.as_str() == cluster,
        // Newer than the emoji data: pictographs, and flags (regional indicator pairs).
        None => cluster.chars().next().is_some_and(|c| matches!(u32::from(c), 0x1_f1e6..=0x1_f1ff | 0x1_f300..=0x1_faff)),
    }
}

/// `text` as a layout job in `format`, with its emoji in [`family`]. Text without emoji is one section.
pub fn job(text: &str, format: TextFormat) -> LayoutJob {
    let mut job = LayoutJob::default();
    if text.is_ascii() {
        job.append(text, 0.0, format);
        return job;
    }
    let emoji_format = TextFormat { font_id: FontId::new(format.font_id.size, family()), ..format.clone() };
    let mut start = 0;
    let mut current = None;
    for (at, cluster) in text.grapheme_indices(true) {
        let emoji = is_emoji(cluster);
        match current {
            Some(was) if was != emoji => {
                job.append(&text[start..at], 0.0, if was { emoji_format.clone() } else { format.clone() });
                start = at;
                current = Some(emoji);
            }
            None => current = Some(emoji),
            Some(_) => {}
        }
    }
    job.append(&text[start..], 0.0, if current == Some(true) { emoji_format } else { format });
    job
}

/// Proportional text at `size` in `color`, with its emoji in colour.
pub fn text(text: &str, size: f32, color: Color32) -> LayoutJob {
    job(text, TextFormat::simple(FontId::proportional(size), color))
}

#[cfg(test)]
mod tests {
    use super::{is_emoji, job};
    use egui::text::TextFormat;

    #[test]
    fn text_stays_text_and_emoji_are_emoji() {
        for text in ["1", "#", "*", "©", "®", "™", "❤", "a", "é", "↔", "\u{2764}\u{fe0e}"] {
            assert!(!is_emoji(text), "{text:?} is text");
        }
        for emoji in ["1\u{fe0f}\u{20e3}", "#\u{fe0f}\u{20e3}", "❤\u{fe0f}", "😀", "👍🏽", "🇩🇰", "👨\u{200d}👩\u{200d}👧", "🏳\u{fe0f}\u{200d}🌈"]
        {
            assert!(is_emoji(emoji), "{emoji:?} is an emoji");
        }
    }

    #[test]
    fn a_job_keeps_the_text_and_puts_emoji_in_their_family() {
        let text = "Hi 1️⃣ and ❤️👍🏽, 1 ❤ too";
        let j = job(text, TextFormat::default());
        assert_eq!(j.text, text, "the text, and so what a screen reader reads, is unchanged");
        let emoji: Vec<&str> = j
            .sections
            .iter()
            .filter(|s| s.format.font_id.family == super::family())
            .map(|s| &j.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert_eq!(emoji, ["1️⃣", "❤️👍🏽"]);
        assert_eq!(job("plain", TextFormat::default()).sections.len(), 1);
        assert_eq!(job("", TextFormat::default()).sections.len(), 1);
    }
}

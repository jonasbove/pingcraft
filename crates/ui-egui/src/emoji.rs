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
use pingcraft_core::EmojiPresentation;
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

/// Is this grapheme cluster an emoji to draw in colour, in a chat on a network whose app draws symbols that are text
/// by default as `presentation` says? Digits, `#` and `*` on their own are text, and so is anything with the text
/// variation selector (U+FE0E); with the emoji variation selector (`❤️`) or as a keycap (`1️⃣`) they are emoji. A
/// symbol that is text by default (`☺`, `❤`, `©`) is an emoji only where the network's app draws it as one.
pub fn is_emoji(cluster: &str, presentation: EmojiPresentation) -> bool {
    if cluster.is_ascii() || cluster.contains('\u{fe0e}') {
        return false;
    }
    if cluster.contains(['\u{fe0f}', '\u{20e3}']) {
        return true;
    }
    match emojis::get(cluster) {
        // An emoji that is shown as text unless asked otherwise comes back fully qualified, with the selector.
        Some(emoji) if emoji.as_str() == cluster => true,
        Some(_) => match presentation {
            EmojiPresentation::Unicode => false,
            EmojiPresentation::EmojiFirst { arrows } => !MARKS.contains(&cluster) && (arrows || !is_arrow(cluster)),
        },
        // Newer than the emoji data: pictographs, and flags (regional indicator pairs).
        None => cluster.chars().next().is_some_and(|c| matches!(u32::from(c), 0x1_f1e6..=0x1_f1ff | 0x1_f300..=0x1_faff)),
    }
}

/// Symbols that stay text without U+FE0F even where a network's app draws other text-default symbols as emoji: in
/// running text they are marks, not pictures (Telegram Desktop keeps exactly these as text).
const MARKS: [&str; 3] = ["\u{a9}", "\u{ae}", "\u{2122}"];

/// The arrows that are text by default and have an emoji form (Unicode's "arrow" emoji subgroup): ↔ ↕ ↖ ↗ ↘ ↙ ↩ ↪
/// ⤴ ⤵ ⬅ ⬆ ⬇ ➡.
fn is_arrow(cluster: &str) -> bool {
    let mut chars = cluster.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            matches!(c, '\u{2194}'..='\u{2199}' | '\u{21a9}' | '\u{21aa}' | '\u{2934}' | '\u{2935}' | '\u{2b05}'..='\u{2b07}' | '\u{27a1}')
        }
        _ => false,
    }
}

/// Emoji among text are drawn this many times the text's size. At the text's own size a colour emoji is barely
/// taller than a capital letter and looks soft beside it; one size up, as Signal draws them, it reads as a picture.
pub const INLINE_SCALE: f32 = 1.3;

/// A message that is only a few emoji ([`LARGE_MAX`]) is drawn this many times the body text's size, as chat apps
/// draw such a message.
pub const LARGE_SCALE: f32 = 2.5;

/// At most this many emoji, and nothing else but white space, make a message large ([`body`]).
pub const LARGE_MAX: usize = 3;

/// How tall each line of text with emoji is, in the text's size: tall enough for an emoji at [`INLINE_SCALE`], so a
/// line with one is as tall as the lines without, and no emoji reaches into the line above or below. Text and emoji
/// are both centred in it. (A test checks the emoji's glyphs fit.)
pub const EMOJI_LINE: f32 = 1.5;

/// How big a job's emoji are.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Scale {
    /// At [`INLINE_SCALE`], in lines [`EMOJI_LINE`] tall.
    Inline,
    /// Everything at [`LARGE_SCALE`]: a message of emoji only.
    Large,
    /// At the size given: emoji on their own, sized by the caller (the picker, the verification symbols).
    AsGiven,
}

/// `text` as a layout job in `format`, with its emoji (as [`is_emoji`] tells them for `presentation`) in [`family`]
/// at [`INLINE_SCALE`]. Text without emoji is one section at `format`; lines with emoji are [`EMOJI_LINE`] tall.
pub fn job(text: &str, format: TextFormat, presentation: EmojiPresentation) -> LayoutJob {
    job_at(text, format, Scale::Inline, presentation)
}

fn job_at(text: &str, format: TextFormat, scale: Scale, presentation: EmojiPresentation) -> LayoutJob {
    let mut job = LayoutJob::default();
    if text.is_ascii() {
        job.append(text, 0.0, format);
        return job;
    }
    let size = format.font_id.size;
    let (format, emoji_size) = match scale {
        Scale::Inline => (TextFormat { line_height: Some(size * EMOJI_LINE), ..format }, size * INLINE_SCALE),
        Scale::Large => (TextFormat { font_id: FontId::new(size * LARGE_SCALE, format.font_id.family.clone()), ..format }, size * LARGE_SCALE),
        Scale::AsGiven => (format, size),
    };
    let emoji_format = TextFormat { font_id: FontId::new(emoji_size, family()), ..format.clone() };
    let mut start = 0;
    let mut current = None;
    for (at, cluster) in text.grapheme_indices(true) {
        let emoji = is_emoji(cluster, presentation);
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
    job.append(&text[start..], 0.0, if current == Some(true) { emoji_format } else { format.clone() });
    if scale == Scale::Inline && current == Some(false) && job.sections.len() == 1 {
        // No emoji after all: the text's own line height.
        job.sections[0].format.line_height = None;
    }
    job
}

/// Proportional text at `size` in `color`, with its emoji in colour at [`INLINE_SCALE`]: names, previews, quotes,
/// reactions. `presentation` is the chat's network's ([`pingcraft_core::Network::emoji_presentation`]).
pub fn text(text: &str, size: f32, color: Color32, presentation: EmojiPresentation) -> LayoutJob {
    job(text, TextFormat::simple(FontId::proportional(size), color), presentation)
}

/// A message's text at `size` in `color`: large ([`LARGE_SCALE`]) when it is only a few emoji ([`is_large`]),
/// otherwise as [`text`].
pub fn body(text: &str, size: f32, color: Color32, presentation: EmojiPresentation) -> LayoutJob {
    let scale = if is_large(text, presentation) { Scale::Large } else { Scale::Inline };
    job_at(text, TextFormat::simple(FontId::proportional(size), color), scale, presentation)
}

/// Emoji (or text with them) at exactly `size`, for places that size emoji themselves: the picker, the quick
/// reactions and the verification symbols. These are emoji PingCraft chose, so Unicode's presentation applies.
pub fn symbol(text: &str, size: f32, color: Color32) -> LayoutJob {
    job_at(text, TextFormat::simple(FontId::proportional(size), color), Scale::AsGiven, EmojiPresentation::Unicode)
}

/// Whether a message is shown large: one to [`LARGE_MAX`] emoji and nothing else but white space.
pub fn is_large(text: &str, presentation: EmojiPresentation) -> bool {
    let mut emoji = 0;
    for cluster in text.graphemes(true).filter(|c| !c.chars().all(char::is_whitespace)) {
        if !is_emoji(cluster, presentation) {
            return false;
        }
        emoji += 1;
        if emoji > LARGE_MAX {
            return false;
        }
    }
    emoji > 0
}

#[cfg(test)]
mod tests {
    use super::{EMOJI_LINE, INLINE_SCALE, LARGE_SCALE, body, is_emoji, is_large, job, symbol, text};
    use egui::Color32;
    use egui::text::{LayoutJob, TextFormat};
    use pingcraft_core::EmojiPresentation::{self, EmojiFirst, Unicode};
    use pingcraft_core::Network;

    /// Each section of a job: its text, its size, whether it is in the emoji family, and its line height.
    fn sections(j: &LayoutJob) -> Vec<(&str, f32, bool, Option<f32>)> {
        j.sections
            .iter()
            .map(|s| {
                (
                    &j.text[s.byte_range.start.0..s.byte_range.end.0],
                    s.format.font_id.size,
                    s.format.font_id.family == super::family(),
                    s.format.line_height,
                )
            })
            .collect()
    }

    #[test]
    fn text_stays_text_and_emoji_are_emoji() {
        for text in ["1", "#", "*", "©", "®", "™", "❤", "a", "é", "↔", "\u{2764}\u{fe0e}"] {
            assert!(!is_emoji(text, Unicode), "{text:?} is text");
        }
        for emoji in ["1\u{fe0f}\u{20e3}", "#\u{fe0f}\u{20e3}", "❤\u{fe0f}", "😀", "👍🏽", "🇩🇰", "👨\u{200d}👩\u{200d}👧", "🏳\u{fe0f}\u{200d}🌈"]
        {
            assert!(is_emoji(emoji, Unicode), "{emoji:?} is an emoji");
        }
    }

    #[test]
    fn a_job_keeps_the_text_and_puts_emoji_in_their_family() {
        let text = "Hi 1️⃣ and ❤️👍🏽, 1 ❤ too";
        let j = job(text, TextFormat::default(), Unicode);
        assert_eq!(j.text, text, "the text, and so what a screen reader reads, is unchanged");
        let emoji: Vec<&str> = j
            .sections
            .iter()
            .filter(|s| s.format.font_id.family == super::family())
            .map(|s| &j.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert_eq!(emoji, ["1️⃣", "❤️👍🏽"]);
        assert_eq!(job("plain", TextFormat::default(), Unicode).sections.len(), 1);
        assert_eq!(job("", TextFormat::default(), Unicode).sections.len(), 1);
    }

    /// Emoji among text are a size up from it; every line of such text is the same height, so a line with an emoji
    /// does not jump. Text without emoji is untouched.
    #[test]
    fn emoji_among_text_are_a_size_up_in_even_lines() {
        let j = text("Hi 👍🏽 there", 14.0, Color32::WHITE, Unicode);
        let line = Some(14.0 * EMOJI_LINE);
        assert_eq!(sections(&j), [("Hi ", 14.0, false, line), ("👍🏽", 14.0 * INLINE_SCALE, true, line), (" there", 14.0, false, line)]);
        assert_eq!(j.text, "Hi 👍🏽 there", "what a screen reader reads is unchanged");
        assert_eq!(sections(&text("Hi é there", 14.0, Color32::WHITE, Unicode)), [("Hi é there", 14.0, false, None)]);
        assert_eq!(sections(&text("plain", 14.0, Color32::WHITE, Unicode)), [("plain", 14.0, false, None)]);
        // Places that size emoji themselves get exactly that size.
        assert_eq!(sections(&symbol("👍", 18.0, Color32::WHITE)), [("👍", 18.0, true, None)]);
    }

    /// A message of one to three emoji, white space aside, is large; four, or any text, is not.
    #[test]
    fn a_message_of_up_to_three_emoji_is_large() {
        for large in ["😀", " 👍🏽 ", "❤\u{fe0f}👨\u{200d}👩\u{200d}👧 🇩🇰", "1\u{fe0f}\u{20e3}\n😀"] {
            assert!(is_large(large, Unicode), "{large:?} is large");
        }
        for not in ["", "  ", "😀😀😀😀", "hi 😀", "😀!", "❤", "©"] {
            assert!(!is_large(not, Unicode), "{not:?} is not large");
        }
        let said = "😀 👍🏽";
        let j = body(said, 14.0, Color32::WHITE, Unicode);
        assert_eq!(j.text, said, "what a screen reader reads is unchanged");
        assert_eq!(
            sections(&j),
            [("😀", 14.0 * LARGE_SCALE, true, None), (" ", 14.0 * LARGE_SCALE, false, None), ("👍🏽", 14.0 * LARGE_SCALE, true, None)]
        );
        // Four is an ordinary message, with its emoji at the inline size.
        let four = body("😀😀😀😀", 14.0, Color32::WHITE, Unicode);
        assert_eq!(sections(&four), [("😀😀😀😀", 14.0 * INLINE_SCALE, true, Some(14.0 * EMOJI_LINE))]);
        assert_eq!(sections(&body("hi 😀", 14.0, Color32::WHITE, Unicode))[1].1, 14.0 * INLINE_SCALE);
    }

    /// A symbol that is text by default is drawn as the chat's network's app draws it (PLAN.md, "Messages and chats"
    /// 1): Signal, WhatsApp and Discord draw ☺ ❤ ✔ as emoji, Telegram arrows too; ©, digits and (but on Telegram)
    /// arrows stay text; U+FE0E keeps any of them text; Matrix follows Unicode.
    #[test]
    fn text_default_symbols_follow_the_networks_app() {
        let cases = ["☺", "❤", "✔", "©", "1", "↔", "☺\u{fe0e}"];
        let drawn = |network: &str| -> Vec<bool> { cases.iter().map(|c| is_emoji(c, Network::from_id(network).emoji_presentation())).collect() };
        assert_eq!(drawn("matrix"), [false, false, false, false, false, false, false]);
        for network in ["signal", "whatsapp", "discord"] {
            assert_eq!(drawn(network), [true, true, true, false, false, false, false], "{network}");
        }
        assert_eq!(drawn("telegram"), [true, true, true, false, false, true, false]);
        assert_eq!(drawn("somewhere-new"), drawn("matrix"));
        // With U+FE0F, an emoji everywhere.
        for presentation in [Unicode, EmojiFirst { arrows: false }, EmojiFirst { arrows: true }] {
            for emoji in ["☺\u{fe0f}", "©\u{fe0f}", "↔\u{fe0f}", "1\u{fe0f}\u{20e3}", "😀"] {
                assert!(is_emoji(emoji, presentation), "{emoji:?} with {presentation:?}");
            }
            assert!(!is_emoji("™", presentation) && !is_emoji("®", presentation));
        }
        // A lone ☺ is a large message on Signal, and text on Matrix.
        let signal = Network::from_id("signal").emoji_presentation();
        assert!(is_large("☺", signal) && !is_large("☺", Unicode));
        let on = |p: EmojiPresentation| sections(&text("so ☺ today", 14.0, Color32::WHITE, p)).len();
        assert_eq!((on(signal), on(Unicode)), (3, 1));
    }
}

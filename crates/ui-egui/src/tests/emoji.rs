//! Colour emoji: what the fonts draw, the picker and `:name:` shortcodes.

use egui::epaint::text::Glyph;

/// The glyphs `text` is drawn with in the app's fonts, laid out as the app lays it out (`colour_family`) or as plain
/// text, and the font atlas they were drawn into.
fn glyphs(text: &str, colour_family: bool) -> (Vec<Glyph>, egui::ColorImage) {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
    out.textures_delta.clear();
    let job = if colour_family {
        crate::emoji::text(text, 28.0, egui::Color32::WHITE, pingcraft_core::EmojiPresentation::Unicode)
    } else {
        egui::text::LayoutJob::simple_singleline(text.to_owned(), egui::FontId::proportional(28.0), egui::Color32::WHITE)
    };
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    let glyphs = galley.rows.iter().flat_map(|r| r.glyphs.iter().cloned()).collect();
    (glyphs, ctx.fonts(|f| f.image()))
}

/// How many pixels of a glyph in the atlas have a colour of their own (not a shade of grey).
fn coloured_pixels(glyph: &Glyph, atlas: &egui::ColorImage) -> usize {
    let [x0, y0] = glyph.uv_rect.min.map(usize::from);
    let [x1, y1] = glyph.uv_rect.max.map(usize::from);
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = atlas.pixels[y * atlas.width() + x];
            let (lo, hi) = (p.r().min(p.g()).min(p.b()), p.r().max(p.g()).max(p.b()));
            p.a() > 0 && hi - lo > 40
        })
        .count()
}

/// Each sequence is one glyph, drawn from the colour face: a joined family, a skin tone, a flag, a keycap and a red
/// heart. The last four are checked for colour in the atlas too (Noto's family is grey silhouettes).
#[test]
#[cfg_attr(not(color_emoji), ignore = "built without the colour emoji font: cargo xtask fonts")]
fn emoji_sequences_are_single_colour_glyphs() {
    for emoji in ["👨\u{200d}👩\u{200d}👧", "👍🏽", "🇩🇰", "1\u{fe0f}\u{20e3}", "❤\u{fe0f}"] {
        let (glyphs, atlas) = glyphs(emoji, true);
        let drawn: Vec<&Glyph> = glyphs.iter().filter(|g| g.uv_rect.max != g.uv_rect.min).collect();
        assert_eq!(drawn.len(), 1, "{emoji:?} is one glyph: {glyphs:?}");
        assert!(drawn[0].is_color, "{emoji:?} is drawn from the colour face");
        if !emoji.starts_with('👨') {
            assert!(coloured_pixels(drawn[0], &atlas) > 100, "{emoji:?} is painted in colour");
        }
    }
}

/// Laid out as plain text, a keycap starts in the text face (egui picks the face by the first character, and the
/// text face has the digits), which is why messages lay emoji out in their own family. (`❤️` would too with a text
/// face that has U+2764; egui's Ubuntu Light has not.)
#[test]
#[cfg_attr(not(color_emoji), ignore = "built without the colour emoji font: cargo xtask fonts")]
fn plain_layout_would_draw_keycaps_as_text() {
    for emoji in ["1\u{fe0f}\u{20e3}", "#\u{fe0f}\u{20e3}"] {
        let (glyphs, _) = glyphs(emoji, false);
        assert!(glyphs.iter().all(|g| !g.is_color), "{emoji:?} in the text family: {glyphs:?}");
    }
}

/// A message with emoji reads the same to a screen reader as it was written: laying emoji out in their own family
/// changes how they are drawn, not the text.
#[test]
fn emoji_in_a_message_keep_their_text() {
    use egui_kittest::kittest::Queryable;
    use pingcraft_core::app::Event;
    use pingcraft_core::{ChatId, MessageBody, SendState};

    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    let said = "Ready 1\u{fe0f}\u{20e3} ❤\u{fe0f} 👨\u{200d}👩\u{200d}👧 🇩🇰 👍🏽";
    let m = super::message("9", "Mei Tanaka", false, MessageBody::Text(said.into()), SendState::Sent);
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: vec![m] }]);
    h.run();
    h.get_by_label(said);
}

/// What the open chat sent since the last call, as text messages and reactions.
fn sent_texts(fake: &super::Fake) -> Vec<String> {
    use pingcraft_core::app::Command;
    fake.take()
        .into_iter()
        .filter_map(|c| match c {
            Command::SendText { text, .. } => Some(format!("text {text}")),
            Command::React { key, .. } => Some(format!("react {key}")),
            _ => None,
        })
        .collect()
}

#[test]
#[cfg_attr(not(color_emoji), ignore = "the picker shows emoji egui's monochrome face lacks: cargo xtask fonts")]
fn the_picker_finds_an_emoji_and_puts_it_in_the_message() {
    use egui_kittest::kittest::Queryable;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    super::type_into(&mut h, "Message Book club", "Great ");
    h.get_by_label("Emoji").click();
    h.run();
    super::type_into(&mut h, "Search emoji", "thumbs up");
    h.get_by_label("thumbs up").click();
    h.run();
    assert!(h.query_by_label("Search emoji").is_none(), "the picker closes");
    h.get_by_label("SEND").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["text Great 👍"]);
    // Recently used, at the top of the picker next time.
    h.get_by_label("Emoji").click();
    h.run();
    h.get_by_label("RECENTLY USED");
}

#[test]
fn a_shortcode_becomes_its_emoji_as_it_is_finished() {
    use egui_kittest::kittest::Queryable;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    super::type_into(&mut h, "Message Book club", "hi :smi");
    // Suggestions while it is typed.
    h.get_by_label("Insert 😄 :smile:");
    super::type_into(&mut h, "Message Book club", "le: all");
    h.get_by_label("SEND").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["text hi 😄 all"]);
}

#[test]
#[cfg_attr(not(color_emoji), ignore = "the picker shows emoji egui's monochrome face lacks: cargo xtask fonts")]
fn a_reaction_can_be_any_emoji_from_the_picker() {
    use egui_kittest::kittest::Queryable;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("More reactions…").click();
    h.run();
    super::type_into(&mut h, "Search emoji", "party popper");
    h.get_by_label("party popper").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["react 🎉"]);
    // Now first in the quick row.
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("React with 🎉");
}

/// The glyphs `text` is drawn with in `ctx`'s fonts, laid out as the app lays emoji out.
fn glyphs_in(ctx: &egui::Context, text: &str) -> Vec<Glyph> {
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let galley = ctx.fonts_mut(|f| f.layout_job(crate::emoji::text(text, 28.0, egui::Color32::WHITE, pingcraft_core::EmojiPresentation::Unicode)));
    galley.rows.iter().flat_map(|r| r.glyphs.iter().cloned()).collect()
}

/// The web build starts without the colour font and fetches it after the page opens: until it arrives emoji are
/// drawn in monochrome, and once it is installed the same text is drawn in colour.
#[test]
#[cfg_attr(not(color_emoji), ignore = "built without the colour emoji font: cargo xtask fonts")]
fn the_colour_font_can_arrive_after_the_first_frame() {
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::emoji::monochrome_definitions());
    assert!(glyphs_in(&ctx, "👍").iter().all(|g| !g.is_color), "monochrome before the font arrives");
    let bytes = crate::emoji::EMBEDDED.map(<[u8]>::to_vec).unwrap_or_default();
    assert_eq!(crate::emoji::install_color_font(&ctx, bytes), Ok(()));
    assert!(glyphs_in(&ctx, "👍").iter().any(|g| g.is_color), "colour once it is installed");
}

/// A file that is not the pinned font (a broken download, an error page, something else served in its place) is
/// refused, and emoji stay as they were.
#[test]
fn a_font_that_is_not_the_pinned_one_is_refused() {
    use crate::emoji::FontRefused;
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::emoji::monochrome_definitions());
    assert_eq!(crate::emoji::install_color_font(&ctx, b"<html>not found</html>".to_vec()), Err(FontRefused::NotPinned));
    assert_eq!(crate::emoji::install_color_font(&ctx, vec![0; 6_000_000]), Err(FontRefused::TooBig));
    assert!(glyphs_in(&ctx, "👍").iter().all(|g| !g.is_color), "still monochrome");
}

/// Whether the backend was told the user used `emoji`, so it joins the account's recently used emoji.
fn told_used(commands: &[pingcraft_core::app::Command], emoji: &str) -> bool {
    commands.iter().any(|c| matches!(c, pingcraft_core::app::Command::EmojiUsed(e) if e == emoji))
}

#[test]
fn an_emoji_used_is_kept_with_the_account() {
    use egui_kittest::kittest::Queryable;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("React with 👍").click();
    h.run();
    assert!(told_used(&fake.take(), "👍"), "a quick reaction");
    super::type_into(&mut h, "Message Book club", "hi :smi");
    h.get_by_label("Insert 😄 :smile:").click();
    h.run();
    assert!(told_used(&fake.take(), "😄"), "a suggested shortcode");
}

/// The recently used emoji come from the account (another device's uses included), and are forgotten on sign-out.
#[test]
fn recently_used_emoji_come_from_the_account() {
    use egui_kittest::kittest::Queryable;
    use pingcraft_core::app::Event;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    fake.tell([Event::RecentEmoji(vec!["🎻".into(), "🎉".into(), "not an emoji".into()])]);
    h.run();
    assert_eq!(crate::picker::recent(&h.ctx), ["🎻", "🎉"], "what PingCraft cannot show is left out");
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("React with 🎻");
    h.get_by_label("React with 🎉");
    fake.tell([Event::SignedOut]);
    h.run();
    assert!(crate::picker::recent(&h.ctx).is_empty(), "the next account does not see them");
}

/// The skin tone chosen in the picker is marked there, applies to the emoji it shows, and is what the app reports
/// to be remembered.
#[test]
#[cfg_attr(not(color_emoji), ignore = "the picker shows emoji egui's monochrome face lacks: cargo xtask fonts")]
fn a_skin_tone_is_chosen_in_the_picker_and_applies_to_its_emoji() {
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::kittest::{NodeT, Queryable};
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.get_by_label("Emoji").click();
    h.run();
    let toggled = |h: &egui_kittest::Harness<'_, crate::PingCraftApp>, label: &str| {
        h.get_by_role_and_label(Role::RadioButton, label).accesskit_node().toggled()
    };
    assert_eq!(toggled(&h, "Default skin tone"), Some(Toggled::True));
    assert_eq!(toggled(&h, "Medium skin tone"), Some(Toggled::False));
    h.get_by_role_and_label(Role::RadioButton, "Medium skin tone").click();
    h.run();
    assert_eq!(toggled(&h, "Medium skin tone"), Some(Toggled::True));
    assert_eq!(toggled(&h, "Default skin tone"), Some(Toggled::False));
    assert_eq!(h.state().skin_tone(), crate::SkinTone::Medium, "the app hands it over to be saved");
    super::type_into(&mut h, "Search emoji", "thumbs up");
    h.get_by_label("thumbs up: medium skin tone").click();
    h.run();
    h.get_by_label("SEND").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["text 👍🏽"]);
}

/// A saved skin tone applies to the quick reactions and to `:name:` shortcodes too; emoji without skin tones are
/// unchanged.
#[test]
fn a_saved_skin_tone_applies_to_quick_reactions_and_shortcodes() {
    use egui_kittest::kittest::Queryable;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.state_mut().set_skin_tone(crate::SkinTone::Dark);
    h.run();
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("React with 🙏🏿").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["react 🙏🏿"]);
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("React with 😂");
    h.key_press(egui::Key::Escape);
    h.run();
    super::type_into(&mut h, "Message Book club", "ok :+1");
    h.get_by_label("Insert 👍🏿 :+1:");
    super::type_into(&mut h, "Message Book club", ": :wave: :smile:");
    h.get_by_label("SEND").click();
    h.run();
    assert_eq!(sent_texts(&fake), ["text ok 👍🏿 👋🏿 😄"]);
}

/// The skin tones the interface asked the backend to save with the account, since the last call.
fn saved_tones(fake: &super::Fake) -> Vec<String> {
    use pingcraft_core::app::Command;
    fake.take()
        .into_iter()
        .filter_map(|c| match c {
            Command::SaveSkinTone(tone) => Some(tone),
            _ => None,
        })
        .collect()
}

/// The skin tone is kept with the account: one another device chose is taken (and marked in the picker) without
/// being sent back, and one chosen here is sent to be saved.
#[test]
#[cfg_attr(not(color_emoji), ignore = "the picker shows emoji egui's monochrome face lacks: cargo xtask fonts")]
fn the_skin_tone_follows_the_account() {
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::kittest::{NodeT, Queryable};
    use pingcraft_core::app::Event;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    fake.tell([Event::SkinTone(Some("dark".into()))]);
    h.run();
    h.run();
    assert_eq!(h.state().skin_tone(), crate::SkinTone::Dark);
    assert!(saved_tones(&fake).is_empty(), "the account's own tone is not sent back");
    h.get_by_label("Emoji").click();
    h.run();
    let toggled = |h: &egui_kittest::Harness<'_, crate::PingCraftApp>, label: &str| {
        h.get_by_role_and_label(Role::RadioButton, label).accesskit_node().toggled()
    };
    assert_eq!(toggled(&h, "Dark skin tone"), Some(Toggled::True));
    h.get_by_role_and_label(Role::RadioButton, "Medium skin tone").click();
    h.run();
    h.run();
    assert_eq!(saved_tones(&fake), ["medium"], "sent once");
    assert_eq!(toggled(&h, "Medium skin tone"), Some(Toggled::True));
}

/// A tone saved on this computer before the account kept one moves up to an account that has none, once; after
/// that, an account without one (another account on this computer, say) starts from the default.
#[test]
fn a_local_skin_tone_moves_up_to_an_account_without_one_once() {
    use pingcraft_core::app::Event;
    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.state_mut().set_local_skin_tone(crate::SkinTone::MediumDark, false);
    h.run();
    assert!(saved_tones(&fake).is_empty(), "nothing until the account has answered");
    fake.tell([Event::SkinTone(None)]);
    h.run();
    h.run();
    assert_eq!(saved_tones(&fake), ["medium-dark"]);
    assert!(h.state().skin_tone_migrated(), "the app remembers it was moved up");
    assert_eq!(h.state().skin_tone(), crate::SkinTone::MediumDark);

    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    h.state_mut().set_local_skin_tone(crate::SkinTone::MediumDark, true);
    fake.tell([Event::SkinTone(None)]);
    h.run();
    h.run();
    assert!(saved_tones(&fake).is_empty(), "moved up once only");
    assert_eq!(h.state().skin_tone(), crate::SkinTone::Default);
}

/// What the interface does when the account says which tone it has: a pure decision, so each case is checked.
#[test]
fn the_account_tone_decides_over_the_local_one() {
    use crate::SkinTone::{Dark, Default, Medium};
    use crate::picker::account_tone;
    // (account, local, migrated) -> (tone to use, send it up)
    assert_eq!(account_tone(Some(Dark), Medium, false), (Dark, false));
    assert_eq!(account_tone(Some(Dark), Medium, true), (Dark, false));
    assert_eq!(account_tone(None, Medium, false), (Medium, true));
    assert_eq!(account_tone(None, Default, false), (Default, false), "nothing to move up");
    assert_eq!(account_tone(None, Medium, true), (Default, false));
}

/// Emoji among text, a size up, fit their lines in the app's fonts: no emoji reaches past its line into the next, and
/// a line with an emoji is as tall as one without, so lines do not jump. Prints the sizes, for tuning.
#[test]
#[cfg_attr(not(color_emoji), ignore = "built without the colour emoji font: cargo xtask fonts")]
fn emoji_a_size_up_fit_even_lines() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    for size in [12.5, 14.0, 16.0] {
        let galley = ctx.fonts_mut(|f| {
            f.layout_job(crate::emoji::text(
                "Hg 😀 ❤\u{fe0f}\nHg only text\n👍🏽 Hg",
                size,
                egui::Color32::WHITE,
                pingcraft_core::EmojiPresentation::Unicode,
            ))
        });
        let heights: Vec<f32> = galley.rows.iter().map(|r| r.size.y).collect();
        assert_eq!(heights.len(), 3);
        assert!(heights.iter().all(|h| *h == heights[0]), "every line is as tall: {heights:?}");
        let text_row = ctx.fonts_mut(|f| f.row_height(&egui::FontId::proportional(size)));
        assert!(heights[0] >= text_row, "a line with emoji is no shorter than plain text's: {heights:?} {text_row}");
        let mut emoji = 0;
        for row in &galley.rows {
            for g in row.glyphs.iter().filter(|g| g.is_color) {
                emoji += 1;
                let (top, bottom) = (g.pos.y + g.uv_rect.offset.y, g.pos.y + g.uv_rect.offset.y + g.uv_rect.size.y);
                eprintln!("size {size}: line {} (text alone {text_row}), emoji drawn {top}..{bottom}", row.size.y);
                assert!(top >= -0.5 && bottom <= row.size.y + 0.5, "the emoji fits its line at {size}: {top}..{bottom} in {}", row.size.y);
                assert!(g.uv_rect.size.y >= size * 1.15, "the emoji is a size up at {size}: {}", g.uv_rect.size.y);
            }
        }
        assert_eq!(emoji, 3);
    }
}

/// A message of only a few emoji is drawn large, and still reads as written.
#[test]
fn an_emoji_only_message_is_large_and_keeps_its_text() {
    use egui_kittest::kittest::{NodeT, Queryable};
    use pingcraft_core::app::Event;
    use pingcraft_core::{ChatId, MessageBody, SendState};

    let fake = super::Fake::default();
    let mut h = super::messages::book_club(&fake);
    let large = "😀 👍🏽";
    let messages = vec![
        super::message("8", "Mei Tanaka", false, MessageBody::Text(large.into()), SendState::Sent),
        super::message("9", "Mei Tanaka", false, MessageBody::Text("Okay".into()), SendState::Sent),
        super::message("10", "Mei Tanaka", false, MessageBody::Text("😀😀😀😀".into()), SendState::Sent),
        // Text by default in Unicode, but an emoji in Telegram's app, so in this Telegram chat too.
        super::message("11", "Mei Tanaka", false, MessageBody::Text("☺".into()), SendState::Sent),
    ];
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages }]);
    h.run();
    let height = |label: &str| {
        let b = h.get_by_label(label).accesskit_node().raw_bounds().unwrap();
        b.y1 - b.y0
    };
    let (large, text, four) = (height(large), height("Okay"), height("😀😀😀😀"));
    assert!(large > text * 2.0, "two emoji are large: {large} against {text}");
    assert!(four < text * 2.0, "four emoji are not: {four} against {text}");
    assert!(height("☺") > text * 2.0, "a lone ☺ is large on Telegram");
}

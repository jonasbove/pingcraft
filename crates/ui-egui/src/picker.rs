//! The emoji picker (search, recently used, then every emoji in Unicode's order), recently used emoji (the account's,
//! kept on the server: `pingcraft_matrix::recent_emoji`), and `:name:`
//! shortcodes in the composer. Emoji names and shortcodes come from the `emojis` crate (Unicode's CLDR names and
//! GitHub's gemoji shortcodes), in English for now.

use egui::{Color32, Id, RichText, Ui};

use crate::emoji;
use crate::theme::Tokens;
use crate::tl;
use crate::widgets::hud;
use craft_ui::display::Shade;

/// How many recently used emoji the picker shows (the account keeps up to 100).
const RECENT_MAX: usize = 24;
/// Emoji per row in the picker's grid.
const PER_ROW: usize = 8;
/// How many shortcode suggestions show over the composer.
const SUGGESTIONS: usize = 6;

/// A skin tone for the emoji that have them (people, hands): one saved default, applied in the picker, the quick
/// reactions and `:name:` shortcodes. Emoji that already carry a tone (a recently used one, say) keep theirs. It is
/// kept with the account, so it follows the user between devices; the desktop app also keeps a copy in its local
/// settings, to start with and for when the server cannot be reached (PLAN.md, "Messages and chats" 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SkinTone {
    #[default]
    Default,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl SkinTone {
    /// Every tone, in the order the picker shows them.
    pub const ALL: [Self; 6] = [Self::Default, Self::Light, Self::MediumLight, Self::Medium, Self::MediumDark, Self::Dark];

    /// The name it is saved under. Never change one: saved settings would lose their tone.
    pub fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Light => "light",
            Self::MediumLight => "medium-light",
            Self::Medium => "medium",
            Self::MediumDark => "medium-dark",
            Self::Dark => "dark",
        }
    }

    /// The tone saved under `id`; the default for a name it does not know.
    pub fn from_id(id: &str) -> Self {
        Self::ALL.into_iter().find(|t| t.id() == id).unwrap_or_default()
    }

    /// Its name in a list of tones (Settings).
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Default => tl!("Default"),
            Self::Light => tl!("Light"),
            Self::MediumLight => tl!("Medium-light"),
            Self::Medium => tl!("Medium"),
            Self::MediumDark => tl!("Medium-dark"),
            Self::Dark => tl!("Dark"),
        }
    }

    /// Its accessible name in the picker.
    fn name(self) -> &'static str {
        match self {
            Self::Default => tl!("Default skin tone"),
            Self::Light => tl!("Light skin tone"),
            Self::MediumLight => tl!("Medium-light skin tone"),
            Self::Medium => tl!("Medium skin tone"),
            Self::MediumDark => tl!("Medium-dark skin tone"),
            Self::Dark => tl!("Dark skin tone"),
        }
    }

    fn data(self) -> emojis::SkinTone {
        match self {
            Self::Default => emojis::SkinTone::Default,
            Self::Light => emojis::SkinTone::Light,
            Self::MediumLight => emojis::SkinTone::MediumLight,
            Self::Medium => emojis::SkinTone::Medium,
            Self::MediumDark => emojis::SkinTone::MediumDark,
            Self::Dark => emojis::SkinTone::Dark,
        }
    }

    /// `emoji` in this tone, when it has skin tones and no tone of its own yet; otherwise `emoji` as it is. Emoji of
    /// several people take the tone for each of them.
    pub fn apply(self, emoji: &str) -> String {
        match emojis::get(emoji) {
            Some(e) if self != Self::Default && e.skin_tone() == Some(emojis::SkinTone::Default) => {
                e.with_skin_tone(self.data()).map_or(emoji, emojis::Emoji::as_str).to_owned()
            }
            _ => emoji.to_owned(),
        }
    }
}

/// What to do when the account says which tone it keeps (`account`, `None` when it keeps none): the tone to use,
/// and whether to send `local` up to the account. A tone chosen on this computer before the account kept one
/// (`migrated` false) moves up once; after that, an account without one starts from the default, so another
/// account signed in on this computer does not inherit it.
pub fn account_tone(account: Option<SkinTone>, local: SkinTone, migrated: bool) -> (SkinTone, bool) {
    match account {
        Some(tone) => (tone, false),
        None if !migrated && local != SkinTone::Default => (local, true),
        None => (SkinTone::Default, false),
    }
}

fn tone_id() -> Id {
    Id::unique("pingcraft-skin-tone")
}

/// The skin tone in use.
pub fn tone(ctx: &egui::Context) -> SkinTone {
    ctx.data(|d| d.get_temp::<SkinTone>(tone_id())).unwrap_or_default()
}

/// Uses this skin tone from now on.
pub fn set_tone(ctx: &egui::Context, tone: SkinTone) {
    ctx.data_mut(|d| d.insert_temp(tone_id(), tone));
}

/// The recently used emoji, newest first, shared by the composer's picker and reactions: the account's (from the
/// server, through `Event::RecentEmoji`), with this device's newest uses on top at once.
#[derive(Clone, Default)]
struct Recent(Vec<String>);

/// Uses not yet passed on to the backend (`Command::EmojiUsed`, sent at the end of the frame).
#[derive(Clone, Default)]
struct Used(Vec<String>);

fn recent_id() -> Id {
    Id::unique("pingcraft-recent-emoji")
}

fn used_id() -> Id {
    Id::unique("pingcraft-used-emoji")
}

/// The recently used emoji, newest first.
pub fn recent(ctx: &egui::Context) -> Vec<String> {
    ctx.data(|d| d.get_temp::<Recent>(recent_id())).unwrap_or_default().0
}

/// The account's recently used emoji, as the backend reported them. Those PingCraft does not know (newer than its
/// emoji data, or not an emoji) are left out; the account keeps them.
pub fn set_recent(ctx: &egui::Context, recent: &[String]) {
    let shown: Vec<String> = recent.iter().filter(|e| emojis::get(e).is_some()).take(RECENT_MAX).cloned().collect();
    ctx.data_mut(|d| d.insert_temp(recent_id(), Recent(shown)));
}

/// Notes that the user used this emoji: first among the recently used at once, and passed on to the backend.
pub fn remember(ctx: &egui::Context, emoji: &str) {
    ctx.data_mut(|d| {
        let recent = &mut d.get_temp_mut_or_default::<Recent>(recent_id()).0;
        recent.retain(|e| e != emoji);
        recent.insert(0, emoji.to_owned());
        recent.truncate(RECENT_MAX);
        d.get_temp_mut_or_default::<Used>(used_id()).0.push(emoji.to_owned());
    });
}

/// The uses since the last call, oldest first.
pub fn take_used(ctx: &egui::Context) -> Vec<String> {
    ctx.data_mut(|d| std::mem::take(&mut d.get_temp_mut_or_default::<Used>(used_id()).0))
}

/// The quick reactions: the recently used emoji first, then the fixed ones in the skin tone, six in all.
pub fn quick_reactions(ctx: &egui::Context) -> Vec<String> {
    let tone = tone(ctx);
    let mut quick: Vec<String> = Vec::new();
    for e in recent(ctx).into_iter().chain(crate::message::QUICK_REACTIONS.iter().map(|e| tone.apply(e))) {
        if quick.len() == crate::message::QUICK_REACTIONS.len() {
            break;
        }
        if !quick.contains(&e) {
            quick.push(e);
        }
    }
    quick
}

/// The emoji whose name or a shortcode contains every word of `query` (all of them, in order, for an empty one).
pub fn search(query: &str) -> Vec<&'static emojis::Emoji> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    emojis::iter().filter(|e| words.iter().all(|w| e.name().contains(w.as_str()) || e.shortcodes().any(|s| s.contains(w.as_str())))).collect()
}

/// An emoji's name, as its button's accessible name.
pub fn name(emoji: &str) -> String {
    emojis::get(emoji).map_or_else(|| emoji.to_owned(), |e| e.name().to_owned())
}

/// One emoji as a button; its accessible name is the emoji's name.
fn emoji_button(ui: &mut Ui, emoji: &str, size: f32) -> bool {
    let response =
        ui.add(egui::Button::new(emoji::symbol(emoji, size, Color32::PLACEHOLDER)).frame(false).min_size(egui::vec2(size + 10.0, size + 8.0)));
    let label = name(emoji);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    response.clicked()
}

/// The skin tone row: a raised hand in each tone, the one in use marked. Changes the tone in use when one is
/// clicked.
fn tone_row(ui: &mut Ui) {
    let mut tone = tone(ui.ctx());
    ui.horizontal(|ui| {
        hud(ui, tl!("Skin tone"), Shade::Muted);
        for each in SkinTone::ALL {
            let on = each == tone;
            let hand = each.apply("✋");
            let response = ui.add(
                egui::Button::new(emoji::symbol(&hand, 16.0, Color32::PLACEHOLDER))
                    .frame(on)
                    .selected(on)
                    .corner_radius(Tokens::get(ui.ctx()).radius_control)
                    .min_size(egui::vec2(26.0, 26.0)),
            );
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, each.name()));
            if response.clicked() {
                tone = each;
            }
        }
    });
    set_tone(ui.ctx(), tone);
}

/// The picker: a search field, the skin tone, the recently used emoji and the grid, in the skin tone. Returns the
/// emoji the user chose. `id` keeps its search text apart from other pickers'.
pub fn picker(ui: &mut Ui, id: Id) -> Option<String> {
    let t = Tokens::get(ui.ctx());
    let search_id = id.with("search");
    let mut query: String = ui.data(|d| d.get_temp(search_id)).unwrap_or_default();
    let mut chosen = None;
    ui.set_width(PER_ROW as f32 * 34.0 + 16.0);
    let field = ui.add(egui::TextEdit::singleline(&mut query).hint_text(tl!("Search emoji")).desired_width(f32::INFINITY));
    field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, tl!("Search emoji")));
    tone_row(ui);
    let tone = tone(ui.ctx());
    if query.is_empty() {
        let recent = recent(ui.ctx());
        if !recent.is_empty() {
            hud(ui, tl!("Recently used"), Shade::Muted);
            ui.horizontal_wrapped(|ui| {
                for e in recent.iter().take(PER_ROW * 2) {
                    if emoji_button(ui, e, 20.0) {
                        chosen = Some(e.clone());
                    }
                }
            });
        }
        hud(ui, tl!("All emoji"), Shade::Muted);
    }
    let found = search(&query);
    if found.is_empty() {
        ui.label(RichText::new(tl!("No emoji found")).size(t.label).color(t.muted));
    }
    let rows = found.len().div_ceil(PER_ROW);
    egui::ScrollArea::vertical().id_salt(id.with("grid")).max_height(240.0).auto_shrink([false, true]).show_rows(ui, 30.0, rows, |ui, range| {
        for row in range {
            ui.horizontal(|ui| {
                for e in found.iter().skip(row * PER_ROW).take(PER_ROW) {
                    let e = tone.apply(e.as_str());
                    if emoji_button(ui, &e, 20.0) {
                        chosen = Some(e);
                    }
                }
            });
        }
    });
    if chosen.is_some() {
        query.clear();
    }
    ui.data_mut(|d| d.insert_temp(search_id, query));
    chosen
}

/// Whether `c` can be part of a shortcode (gemoji's are lower case letters, digits, `_`, `+` and `-`).
fn shortcode_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '+' | '-')
}

/// A shortcode being typed just before `cursor` (a char index): the char index of its opening `:` and what follows
/// it. The `:` starts the text or follows a space, so times such as `10:30` are not taken for one.
pub fn partial_shortcode(text: &str, cursor: usize) -> Option<(usize, String)> {
    let before: Vec<char> = text.chars().take(cursor).collect();
    let colon = before.iter().rposition(|c| *c == ':')?;
    let word: String = before[colon + 1..].iter().collect();
    let starts_word = colon == 0 || before[colon - 1].is_whitespace();
    (starts_word && !word.is_empty() && word.chars().all(shortcode_char)).then_some((colon, word))
}

/// Every finished `:name:` in the text replaced by its emoji in the skin tone (typed, pasted or committed by an input
/// method in one go), with the cursor (a char index) moved to match. `None` when there is none.
pub fn complete_shortcode(text: &str, cursor: usize, tone: SkinTone) -> Option<(String, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut new_cursor, mut changed) = (0, cursor, false);
    while i < chars.len() {
        let starts_word = i == 0 || chars[i - 1].is_whitespace();
        let end = (chars[i] == ':' && starts_word)
            .then(|| chars[i + 1..].iter().position(|c| !shortcode_char(*c)).map(|n| i + 1 + n))
            .flatten()
            .filter(|&end| end > i + 1 && chars[end] == ':');
        let found =
            end.and_then(|end| emojis::get_by_shortcode(&chars[i + 1..end].iter().collect::<String>()).map(|e| (end, tone.apply(e.as_str()))));
        let Some((end, emoji)) = found else {
            out.push(chars[i]);
            i += 1;
            continue;
        };
        out.push_str(&emoji);
        let (taken, put) = (end + 1 - i, emoji.chars().count());
        if cursor > end {
            new_cursor = new_cursor + put - taken;
        } else if cursor > i {
            new_cursor = out.chars().count();
        }
        changed = true;
        i = end + 1;
    }
    changed.then_some((out, new_cursor))
}

/// Shortcodes starting with what is being typed, with their emoji in the skin tone.
pub fn suggestions(word: &str, tone: SkinTone) -> Vec<(&'static str, String)> {
    if word.chars().count() < 2 {
        return Vec::new();
    }
    let mut found: Vec<(&'static str, &'static str)> =
        emojis::iter().flat_map(|e| e.shortcodes().filter(|s| s.starts_with(word)).map(move |s| (s, e.as_str()))).collect();
    found.sort_by_key(|(s, _)| (s.len(), *s));
    found.truncate(SUGGESTIONS);
    found.into_iter().map(|(s, e)| (s, tone.apply(e))).collect()
}

#[cfg(test)]
mod tests {
    use super::{SkinTone, complete_shortcode, partial_shortcode, search, suggestions};

    #[test]
    fn a_finished_shortcode_becomes_its_emoji() {
        assert_eq!(complete_shortcode("hi :smile:", 10, SkinTone::Default), Some(("hi 😄".to_owned(), 4)));
        assert_eq!(complete_shortcode(":+1: there", 4, SkinTone::Default), Some(("👍 there".to_owned(), 1)));
        assert_eq!(complete_shortcode("hi :nope_not_one:", 17, SkinTone::Default), None);
        assert_eq!(complete_shortcode("at 10:30:", 9, SkinTone::Default), None, "a time is not a shortcode");
        assert_eq!(complete_shortcode("hi :smile", 9, SkinTone::Default), None, "not finished");
        assert_eq!(complete_shortcode("hi :smile: all", 14, SkinTone::Default), Some(("hi 😄 all".to_owned(), 8)), "typed or pasted in one go");
        assert_eq!(complete_shortcode(":wave: :smile:x", 0, SkinTone::Default), Some(("👋 😄x".to_owned(), 0)));
    }

    #[test]
    fn a_shortcode_being_typed_is_found_and_suggested() {
        assert_eq!(partial_shortcode("hi :smi", 7), Some((3, "smi".to_owned())));
        assert_eq!(partial_shortcode("hi :smi", 5), Some((3, "s".to_owned())));
        assert_eq!(partial_shortcode("10:3", 4), None);
        assert_eq!(partial_shortcode("hi : x", 6), None);
        assert!(
            suggestions("smi", SkinTone::Default).iter().any(|(s, e)| *s == "smile" && *e == "😄"),
            "{:?}",
            suggestions("smi", SkinTone::Default)
        );
        assert!(suggestions("s", SkinTone::Default).is_empty());
    }

    #[test]
    fn search_matches_names_and_shortcodes() {
        assert!(search("thumbs up").iter().any(|e| e.as_str() == "👍"));
        assert!(search("+1").iter().any(|e| e.as_str() == "👍"));
        assert!(search("").len() > 1000);
        assert!(search("zzzz qqqq").is_empty());
    }
}

#[cfg(test)]
mod tone_tests {
    use super::SkinTone;

    #[test]
    fn a_tone_applies_only_to_emoji_with_skin_tones() {
        assert_eq!(SkinTone::Medium.apply("👍"), "👍🏽");
        assert_eq!(SkinTone::Dark.apply("👩\u{200d}❤\u{fe0f}\u{200d}👨"), "👩🏿\u{200d}❤\u{fe0f}\u{200d}👨🏿", "both people");
        assert_eq!(SkinTone::Medium.apply("😄"), "😄");
        assert_eq!(SkinTone::Medium.apply("❤\u{fe0f}"), "❤\u{fe0f}");
        assert_eq!(SkinTone::Medium.apply("👍🏻"), "👍🏻", "an emoji with a tone of its own keeps it");
        assert_eq!(SkinTone::Default.apply("👍"), "👍");
        assert_eq!(SkinTone::Medium.apply("not an emoji"), "not an emoji");
    }

    #[test]
    fn a_tone_is_saved_by_a_name_that_does_not_change() {
        let names: Vec<&str> = SkinTone::ALL.iter().map(|t| t.id()).collect();
        assert_eq!(names, ["default", "light", "medium-light", "medium", "medium-dark", "dark"]);
        for tone in SkinTone::ALL {
            assert_eq!(SkinTone::from_id(tone.id()), tone);
        }
        assert_eq!(SkinTone::from_id("purple"), SkinTone::Default, "an unknown name is the default");
    }
}

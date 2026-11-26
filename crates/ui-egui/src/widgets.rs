//! The pieces the screens are made of: craft-ui's components (buttons, fields, check boxes, badges, text in type
//! roles, icons) wherever one fits, and a few small pieces of PingCraft's own (the avatar, the sheet's frame) drawn
//! from the theme's tokens. Every control has an accessible name that starts with its visible label.

use egui::{Align2, CornerRadius, FontId, Frame, Margin, Response, Sense, Stroke, TextureHandle, Ui, Vec2};
use pingcraft_core::{Accent, Alerts, Network};

use craft_behavior::IconId;
use craft_theme::variant::{Emphasis, Tone};
use craft_ui::buttons::{Button, ButtonOutput, IconButton};
use craft_ui::choice::Checkbox;
use craft_ui::contract::{Bind, EditResponse, PopupKind, Props as _, Show as _, Sizing};
use craft_ui::display::{Badge, Icon, Shade, Text};
use craft_ui::primitives::IconSpec;
use craft_ui::style::{TypeRole, Variant};
use craft_ui::text::TextField;

use crate::i18n::fmt;
use crate::theme::Tokens;
use crate::tl;

/// The craft-ui icons (its catalogue, `craft_ui::primitives::icon_names`; the default set is Lucide) PingCraft uses.
/// Where the catalogue has no icon for the meaning, the nearest one stands in (PLAN.md, "craft-libs: what PingCraft
/// lacks").
pub mod icon {
    /// What else can be done with something: a message's or a chat's menu.
    pub const MORE: &str = "more";
    /// Close or cancel: a reply or edit in progress.
    pub const CLOSE: &str = "close";
    /// The next one: the next pinned message.
    pub const NEXT: &str = "chevron_right";
    /// A chat pinned to the top of the inbox.
    pub const PINNED: &str = "pin";
    /// A muted chat. Stand-in: the catalogue has no "notifications off" (bell with a slash); `track_mute` is a muted
    /// speaker.
    pub const MUTED: &str = "track_mute";
}

/// A craft-ui icon button showing `icon` (one of [`icon`]), named `label` for screen readers and in its tooltip.
pub fn icon_button(ui: &mut Ui, icon: &'static str, label: &str) -> ButtonOutput {
    IconButton::new(IconId::new(icon)).label(label.to_owned()).show(ui)
}

/// A craft-ui icon button showing `icon` that opens a menu ([`menu_under`]): screen readers are told it has one.
pub fn menu_button(ui: &mut Ui, icon: &'static str, label: &str) -> ButtonOutput {
    IconButton::new(IconId::new(icon)).label(label.to_owned()).popup(PopupKind::Menu).show(ui)
}

/// A menu that opens under `button` when it is clicked (by the framework's click rule, which a key press or an
/// AccessKit action also meets) and closes when it is clicked again.
pub fn menu_under(button: &ButtonOutput) -> egui::Popup<'_> {
    egui::Popup::menu(&button.response).open_memory(button.clicked().then_some(egui::SetOpenCommand::Toggle))
}

/// A craft-ui icon in the muted ink, named `label` for screen readers and in its tooltip: a mark such as a chat's pin.
pub fn mark_icon(ui: &mut Ui, icon: &'static str, label: &str) -> Response {
    Icon::new(IconSpec::svg(IconId::new(icon))).label(label.to_owned()).show(ui)
}

/// A small label over a group of controls: caption text, in the normal or the muted shade.
pub fn hud(ui: &mut Ui, text: &str, shade: Shade) -> Response {
    Text::new(text.to_owned()).role(TypeRole::Caption).shade(shade).show(ui)
}

/// Which of craft-ui's push buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The one thing to do next (`Variant::PRIMARY`).
    Primary,
    /// Everything else (`Variant::SECONDARY`).
    Secondary,
    /// What cannot be undone (`Variant::DANGER`).
    Danger,
}

/// A craft-ui push button; its accessible name is its label. Read a click with the output's `clicked`, which a key
/// press or an AccessKit action also meets.
pub fn button(ui: &mut Ui, text: &str, kind: Kind, enabled: bool) -> ButtonOutput {
    let variant = match kind {
        Kind::Primary => Variant::PRIMARY,
        Kind::Secondary => Variant::SECONDARY,
        Kind::Danger => Variant::DANGER,
    };
    Button::new(text.to_owned()).variant(variant).enabled(enabled).show(ui)
}

/// A labelled craft-ui text field (masked for a password), with its label over it. The label names the field for
/// screen readers too. The output says whether Enter submitted it.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, password: bool, hint: &str) -> EditResponse<String> {
    hud(ui, label, Shade::Normal);
    let mut field = TextField::new(label.to_owned(), Bind::bound(value)).sizing(Sizing::FillWidth);
    if !hint.is_empty() {
        field = field.placeholder(hint.to_owned());
    }
    if password {
        field = field.masked();
    }
    field.show(ui)
}

/// A few words in muted text, wrapped.
pub fn note(ui: &mut Ui, text: &str) -> Response {
    Text::new(text.to_owned()).shade(Shade::Muted).show(ui)
}

/// A line in the colour that says something is wrong.
pub fn problem(ui: &mut Ui, text: &str) -> Response {
    Text::new(text.to_owned()).tone(Tone::Bad).show(ui)
}

/// A network's name as people write it. Networks are proper names and are not translated.
pub fn network_name(network: &Network) -> String {
    match network.id() {
        "telegram" => "Telegram".into(),
        "signal" => "Signal".into(),
        "whatsapp" => "WhatsApp".into(),
        "discord" => "Discord".into(),
        "matrix" => "Matrix".into(),
        other => other.to_owned(),
    }
}

/// The network tag: the name in capitals, outlined in the network's colour over a wash of it (Matrix neutral).
pub fn network_tag(ui: &mut Ui, network: &Network) -> Response {
    network_tag_in(ui, network, None)
}

/// The network tag in a chosen accent colour (a chat's or its network's), or the network's own: craft-ui's neutral
/// badge with the network's name, after a dot in the network's colour (Matrix has none). craft-ui's badges take a tone,
/// not a colour from data, so the dot carries the colour (PLAN.md, "craft-libs: what PingCraft lacks"); the name
/// says which network it is, so colour is never the only cue.
pub fn network_tag_in(ui: &mut Ui, network: &Network, accent: Option<Accent>) -> Response {
    let t = Tokens::get(ui.ctx());
    let colour = accent.map(|a| t.accent_colour(a)).or_else(|| t.network(network));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.caption / 3.0;
        if let Some(colour) = colour {
            let side = t.caption * 0.75;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
            ui.painter().circle_filled(rect.center(), side / 2.0, colour);
        }
        Badge::new(network_name(network)).show(ui)
    })
    .inner
}

/// The accent colour's name, as the network colour it is.
pub fn accent_name(accent: Accent) -> String {
    fmt(tl!("{network}'s"), &[("network", &network_name(&Network::from_id(accent.id())))])
}

/// The notification sound and accent colour controls, for a chat (`what` empty) or a network (`what` its name, in
/// the accessible names). `own` is what is set at this level, `shown` what applies once the levels above are taken
/// in. Returns the new settings for this level when the user changed one.
pub fn alerts_controls(ui: &mut Ui, own: Alerts, shown: Alerts, what: &str, inherit: &str) -> Option<Alerts> {
    let t = Tokens::get(ui.ctx());
    let mut changed = None;
    let for_what =
        |label: &str| if what.is_empty() { label.to_owned() } else { fmt(tl!("{setting} for {what}"), &[("setting", label), ("what", what)]) };
    let was = shown.sounds();
    let mut on = was;
    Checkbox::new(Bind::bound(&mut on)).label(for_what(tl!("Notification sound"))).show(ui);
    if on != was {
        changed = Some(Alerts { sound: Some(on), ..own });
    }
    if what.is_empty() && own.sound.is_some() && button(ui, tl!("Use the network's sound setting"), Kind::Secondary, true).clicked() {
        changed = Some(Alerts { sound: None, ..own });
    }
    ui.horizontal(|ui| {
        hud(ui, tl!("Accent colour"), Shade::Muted);
        let prefix = for_what(tl!("Accent colour"));
        let choices = std::iter::once(None).chain(Accent::ALL.into_iter().map(Some));
        for choice in choices {
            let selected = own.accent == choice;
            let colour = choice.map_or(t.muted, |a| t.accent_colour(a));
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
            let painter = ui.painter();
            match choice {
                Some(_) => {
                    painter.rect_filled(rect.shrink(3.0), CornerRadius::ZERO, colour);
                }
                None => {
                    painter.rect_stroke(rect.shrink(3.0), CornerRadius::ZERO, Stroke::new(1.0, colour), egui::StrokeKind::Inside);
                }
            }
            if selected {
                painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.5, t.ink_strong), egui::StrokeKind::Inside);
            }
            let name = choice.map_or_else(|| inherit.to_owned(), accent_name);
            let label = format!("{prefix}: {name}");
            let response = response.on_hover_text(&name);
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, &label));
            if response.clicked() {
                changed = Some(Alerts { accent: choice, ..changed.unwrap_or(own) });
            }
        }
    });
    changed
}

/// The initials of a name, for an avatar: the first letters of the first two words.
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().filter(|w| w.chars().next().is_some_and(char::is_alphanumeric)).collect();
    let pick = match words.as_slice() {
        [] => name.trim().trim_start_matches(['@', '#', '!']).chars().take(2).collect::<String>(),
        [one] => one.chars().take(2).collect(),
        [first, second, ..] => first.chars().take(1).chain(second.chars().take(1)).collect(),
    };
    pick.to_uppercase()
}

/// A round avatar with initials, in the accent's soft fill. Decorative: the name next to it is what screen readers read.
pub fn avatar(ui: &mut Ui, name: &str, size: f32) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, t.accent_tint);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, initials(name), FontId::proportional(size * 0.36), t.accent_ink);
}

/// The unread count on a chat row: craft-ui's count badge, solid in the accent; `quiet` for a muted chat (the neutral
/// soft badge).
pub fn unread(ui: &mut Ui, count: u32, quiet: bool) {
    let badge = Badge::count(count);
    if quiet {
        badge.show(ui);
    } else {
        badge.variant(Variant::look(Emphasis::Solid, Tone::Accent)).show(ui);
    }
}

/// The mark on a chat the user marked unread: a small accent dot where the count goes.
pub fn marked_unread(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let side = t.caption * 0.8;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    ui.painter().circle_filled(rect.center(), side / 2.0, t.accent);
}

/// The logo, decoded once per context.
pub fn logo(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::unique("pingcraft-logo");
    if let Some(found) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
        return found;
    }
    let decoded = image::load_from_memory_with_format(crate::LOGO_PNG, image::ImageFormat::Png).ok().map(|img| {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        ctx.load_texture("pingcraft-logo", egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(id, decoded.clone()));
    decoded
}

/// The logo at a size, or nothing if it could not be decoded.
pub fn mark(ui: &mut Ui, size: f32) {
    if let Some(texture) = logo(ui.ctx()) {
        ui.add(egui::Image::new(&texture).fit_to_exact_size(Vec2::splat(size)).alt_text("PingCraft"));
    }
}

/// A sheet over the dimmed window, on the theme's raised surface: the logo with a title and a caption, then the body,
/// then a row of actions with the secondary one on the left and the primary on the right. An egui `Modal` until the
/// dialogs move onto craft-ui's dialog host (later).
pub fn sheet(ctx: &egui::Context, id: &str, width: f32, (title, caption): (&str, &str), body: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ctx);
    let pad = t.body * 1.5;
    let frame =
        Frame::new().fill(t.raised).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::from(t.radius_surface)).inner_margin(Margin::ZERO);
    egui::Modal::new(egui::Id::unique(id)).frame(frame).backdrop_color(t.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        let margin = Margin::same(pad as i8);
        Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.horizontal(|ui| {
                mark(ui, t.heading * 2.0);
                ui.vertical(|ui| {
                    Text::new(title.to_owned()).role(TypeRole::Heading).show(ui);
                    Text::new(caption.to_owned()).role(TypeRole::Caption).shade(Shade::Muted).show(ui);
                });
            });
        });
        Frame::new().inner_margin(Margin { top: 0, ..margin }).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = t.body * 0.75;
            body(ui);
        });
        let row = Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                actions(ui);
            });
        });
        let r = row.response.rect;
        ui.painter().hline(r.x_range(), r.top(), t.hairline());
    });
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_come_from_the_first_two_words() {
        assert_eq!(initials("Book club"), "BC");
        assert_eq!(initials("lars"), "LA");
        assert_eq!(initials("@ana:example.org"), "AN");
        assert_eq!(initials("  "), "");
        assert_eq!(initials("Ærø Øst"), "ÆØ");
    }
}

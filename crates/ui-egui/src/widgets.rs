//! The pieces the screens are made of, in the design's language: hairlines, square corners, mono capitals for
//! labels and buttons, the orchid for what needs attention. Every control has an accessible name.

use egui::{Align2, Color32, CornerRadius, FontId, Frame, Margin, Response, RichText, Sense, Stroke, TextureHandle, Ui, Vec2};
use pingcraft_core::Network;

use crate::theme::{Tokens, wash};

/// A small mono capitals label (the design's HUD style).
pub fn hud(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.label(RichText::new(text.to_uppercase()).monospace().size(11.0).color(color))
}

/// Which of the design's buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Inverted: the one thing to do next.
    Primary,
    /// A hairline box.
    Secondary,
    /// Filled in the colour that says it cannot be undone.
    Danger,
}

/// A button with a mono capitals label. Its accessible name is the label as shown.
pub fn button(ui: &mut Ui, text: &str, kind: Kind, enabled: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let label = text.to_uppercase();
    let (fill, ink, stroke) = match kind {
        Kind::Primary => (t.invert_bg, t.invert_fg, Stroke::new(1.0, t.invert_bg)),
        Kind::Secondary => (Color32::TRANSPARENT, t.ink, Stroke::new(1.0, t.line_strong)),
        Kind::Danger => (t.bad, t.bg, Stroke::new(1.0, t.bad)),
    };
    let hover = match kind {
        Kind::Primary => t.invert_bg.lerp_to_gamma(t.accent, 0.35),
        Kind::Secondary => t.line,
        Kind::Danger => t.bad.lerp_to_gamma(t.ink_strong, 0.2),
    };
    let button = egui::Button::new(RichText::new(label).monospace().strong().size(11.0).color(ink))
        .stroke(stroke)
        .corner_radius(CornerRadius::ZERO)
        .min_size(Vec2::new(0.0, 34.0));
    animated(ui, button, enabled, fill, hover)
}

/// Adds a button whose fill eases from `fill` to `hover` while the pointer is over it (120 ms each way). The fill is
/// painted underneath the button, so the button's own fill is cleared.
pub fn animated(ui: &mut Ui, button: egui::Button<'_>, enabled: bool, fill: Color32, hover: Color32) -> Response {
    let slot = ui.painter().add(egui::Shape::Noop);
    let response = ui.add_enabled(enabled, button.fill(Color32::TRANSPARENT));
    let k = ui.ctx().animate_bool_with_time(response.id.with("hover"), enabled && response.hovered(), 0.12);
    ui.painter().set(slot, egui::Shape::rect_filled(response.rect, CornerRadius::ZERO, fill.lerp_to_gamma(hover, k)));
    response
}

/// A labelled text field. The label names the field for screen readers.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, password: bool, hint: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let label = hud(ui, label, t.ink);
    let edit = egui::TextEdit::singleline(value).password(password).hint_text(hint).desired_width(f32::INFINITY).margin(Margin::symmetric(10, 8));
    ui.add(edit).labelled_by(label.id)
}

/// A few words in muted text, wrapped.
pub fn note(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text).size(12.5).color(t.muted))
}

/// A line in the colour that says something is wrong.
pub fn problem(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(text).size(13.0).color(t.bad))
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
    let t = Tokens::get(ui.ctx());
    let (ink, fill, line) = match t.network(network) {
        Some(c) => (c, wash(c), c),
        None => (t.muted, Color32::TRANSPARENT, t.line_strong),
    };
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, line))
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| ui.label(RichText::new(network_name(network).to_uppercase()).monospace().size(10.0).color(ink)))
        .inner
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

/// A square avatar with initials. Decorative: the name next to it is what screen readers read.
pub fn avatar(ui: &mut Ui, name: &str, size: f32) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, t.line_strong), egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, initials(name), FontId::monospace(size * 0.33), t.ink);
}

/// The unread count on a chat row.
pub fn unread(ui: &mut Ui, count: u32) {
    let t = Tokens::get(ui.ctx());
    Frame::new().fill(t.accent).inner_margin(Margin::symmetric(6, 1)).show(ui, |ui| {
        ui.label(RichText::new(count.to_string()).monospace().size(10.0).color(t.on_accent));
    });
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

/// A sheet over the dimmed window: a hero with the logo, a title and a caption, then the body, then a row of
/// actions with the secondary one on the left and the primary on the right.
pub fn sheet(ctx: &egui::Context, id: &str, width: f32, (title, caption): (&str, &str), body: impl FnOnce(&mut Ui), actions: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ctx);
    let frame = Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.line_strong)).inner_margin(Margin::ZERO);
    egui::Modal::new(egui::Id::unique(id)).frame(frame).backdrop_color(t.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 0.0;
        let hero = Frame::new().fill(t.accent_tint).inner_margin(Margin { left: 24, right: 24, top: 18, bottom: 16 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                mark(ui, 44.0);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(26.0).strong().color(t.ink_strong));
                    hud(ui, caption, t.muted);
                });
            });
        });
        // The 2 px orchid rule on top of the hero.
        let top = hero.response.rect;
        ui.painter().line_segment([top.left_top(), top.right_top()], Stroke::new(2.0, t.accent));
        ui.painter().hline(top.x_range(), top.bottom(), t.hairline());
        Frame::new().inner_margin(Margin::same(24)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            body(ui);
        });
        let row = Frame::new().inner_margin(Margin::symmetric(24, 14)).show(ui, |ui| {
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

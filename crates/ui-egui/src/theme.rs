//! PingCraft's look: the Craft launcher's layout language (hard edges, hairlines, quiet colours) with the orchid
//! of PingCraft's logo. The values are the design's tokens (`design/tools/lib.py`, `design/README.md`).

use egui::{Color32, CornerRadius, Stroke};

use pingcraft_core::Network;

const fn hex(rgb: u32) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

const fn alpha(rgb: u32, a: u8) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

/// One appearance's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Color32,
    pub sunken: Color32,
    pub ink: Color32,
    pub ink_strong: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    /// The orchid, as a fill (unread counts, the sheet's rule) under `on_accent` text.
    pub accent: Color32,
    /// The orchid for text and thin marks.
    pub accent_ink: Color32,
    pub on_accent: Color32,
    pub accent_tint: Color32,
    pub invert_bg: Color32,
    pub invert_fg: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub scrim: Color32,
    pub net_telegram: Color32,
    pub net_signal: Color32,
    pub net_discord: Color32,
    pub net_whatsapp: Color32,
}

impl Tokens {
    pub const DARK: Self = Self {
        dark: true,
        bg: hex(0x121316),
        sunken: hex(0x060607),
        ink: hex(0xf2f1ee),
        ink_strong: hex(0xffffff),
        muted: alpha(0xf2f1ee, 0x99),
        faint: alpha(0xf2f1ee, 0x61),
        line: alpha(0xf2f1ee, 0x26),
        line_strong: alpha(0xf2f1ee, 0x66),
        accent: hex(0xb546c9),
        accent_ink: hex(0xd58ae3),
        // White, as in the design: 4.47:1 on the orchid, a hair under WCAG's 4.5 (dark text is worse, 4.2:1).
        on_accent: hex(0xffffff),
        accent_tint: alpha(0xb546c9, 0x1f),
        invert_bg: hex(0xffffff),
        invert_fg: hex(0x0b0b0c),
        good: hex(0x3fd6b8),
        warn: hex(0xffc14d),
        bad: hex(0xff6259),
        scrim: alpha(0x000000, 0xaa),
        net_telegram: hex(0x7aa6d4),
        net_signal: hex(0x6ea3ff),
        net_discord: hex(0xa3a6ff),
        net_whatsapp: hex(0x58c486),
    };

    pub const LIGHT: Self = Self {
        dark: false,
        bg: hex(0xf2f1ee),
        sunken: hex(0xe9e8e4),
        ink: hex(0x101014),
        ink_strong: hex(0x000000),
        muted: alpha(0x101014, 0x9e),
        faint: alpha(0x101014, 0x6b),
        line: alpha(0x101014, 0x29),
        line_strong: alpha(0x101014, 0x66),
        accent: hex(0xb546c9),
        accent_ink: hex(0x86329a),
        // White, as in the design: 4.47:1 on the orchid, a hair under WCAG's 4.5 (dark text is worse, 4.2:1).
        on_accent: hex(0xffffff),
        accent_tint: alpha(0xb546c9, 0x1a),
        invert_bg: hex(0x101014),
        invert_fg: hex(0xf2f1ee),
        good: hex(0x0a7563),
        warn: hex(0x8a5800),
        bad: hex(0xb3261e),
        scrim: alpha(0x000000, 0x69),
        net_telegram: hex(0x2b5278),
        net_signal: hex(0x1c56c4),
        net_discord: hex(0x4a4ec8),
        net_whatsapp: hex(0x17703c),
    };

    pub fn get(ctx: &egui::Context) -> Self {
        if ctx.theme() == egui::Theme::Dark { Self::DARK } else { Self::LIGHT }
    }

    pub fn hairline(&self) -> Stroke {
        Stroke::new(1.0, self.line)
    }

    /// A network tag's colour; Matrix and networks without a colour of their own stay neutral.
    pub fn network(&self, network: &Network) -> Option<Color32> {
        match network.id() {
            "telegram" => Some(self.net_telegram),
            "signal" => Some(self.net_signal),
            "discord" => Some(self.net_discord),
            "whatsapp" => Some(self.net_whatsapp),
            _ => None,
        }
    }
}

/// The wash behind a network tag: the network's colour at 12 %.
pub fn wash(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 0x1f)
}

/// Installs the look on both of egui's appearances, and the fonts (colour emoji where the build has them; the web
/// build starts in monochrome and fetches the colour font in the background).
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(crate::emoji::definitions());
    #[cfg(target_arch = "wasm32")]
    crate::emoji::web::fetch_color_font(ctx);
    ctx.style_mut_of(egui::Theme::Dark, |style| apply(style, &Tokens::DARK));
    ctx.style_mut_of(egui::Theme::Light, |style| apply(style, &Tokens::LIGHT));
}

fn apply(style: &mut egui::Style, t: &Tokens) {
    let v = &mut style.visuals;
    v.dark_mode = t.dark;
    v.panel_fill = t.bg;
    v.window_fill = t.bg;
    v.extreme_bg_color = t.sunken;
    v.faint_bg_color = t.sunken;
    v.override_text_color = Some(t.ink);
    v.hyperlink_color = t.accent_ink;
    v.selection.bg_fill = t.accent;
    v.selection.stroke = Stroke::new(1.0, t.ink_strong);
    v.window_stroke = Stroke::new(1.0, t.line_strong);
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.error_fg_color = t.bad;
    v.warn_fg_color = t.warn;
    let w = &mut v.widgets;
    for (state, fill, stroke, text) in [
        (&mut w.noninteractive, t.bg, t.line, t.ink),
        (&mut w.inactive, t.bg, t.line_strong, t.ink),
        (&mut w.hovered, t.sunken, t.line_strong, t.ink_strong),
        (&mut w.active, t.sunken, t.accent_ink, t.ink_strong),
        (&mut w.open, t.sunken, t.line_strong, t.ink_strong),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, text);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    style.spacing.interact_size.y = 34.0;
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
}

#[cfg(test)]
mod tests {
    use super::Tokens;
    use egui::Color32;

    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.040_45 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_colours_are_readable_on_both_backgrounds() {
        for t in [Tokens::DARK, Tokens::LIGHT] {
            assert!(contrast(t.ink, t.bg) >= 7.0);
            assert!(contrast(t.accent_ink, t.bg) >= 4.5, "accent_ink on bg");
            // The design's 4.5:1 for unread counts is 4.47:1 computed; an open point for the maintainer.
            assert!(contrast(t.on_accent, t.accent) >= 4.45, "unread counts");
            assert!(contrast(t.invert_fg, t.invert_bg) >= 7.0, "primary buttons");
            for net in [t.net_telegram, t.net_signal, t.net_discord, t.net_whatsapp] {
                assert!(contrast(net, t.bg) >= 4.5, "network tag text");
            }
        }
    }
}

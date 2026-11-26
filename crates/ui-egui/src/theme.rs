//! PingCraft's colours, sizes and type, read from craft-ui's theme (maintainer, 2026-11-26: craft-libs dictates the
//! look). craft-ui's components draw themselves; [`Tokens`] carries the same resolved tokens to the parts PingCraft
//! still draws with egui (panels, the message list, sheets, text fields), and [`apply`] writes them into egui's own
//! styles. That bridge is a stopgap: craft-ui keeps its resolved tokens for its components (`Env::tokens` is not part
//! of its public API) and has no way to give an app's own egui code its theme (PLAN.md, "craft-libs: what PingCraft
//! lacks"). The network colours are data, not theme: they say which network a chat is on, in every theme.

use egui::{Color32, CornerRadius, Stroke};

use craft_theme::{Mode, Rgba};
use craft_ui::runtime::env::Env;
use pingcraft_core::{Accent, Network};

const fn hex(rgb: u32) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

fn colour(rgba: Rgba) -> Color32 {
    craft_theme::Srgba::from(rgba).into()
}

/// The resolved theme's values PingCraft's own egui code paints with, in the mode craft-ui resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Panels and bars.
    pub bg: Color32,
    /// The window's background, and fields.
    pub sunken: Color32,
    /// Popups and sheets.
    pub raised: Color32,
    pub ink: Color32,
    pub ink_strong: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    /// The accent, as a fill under `on_accent` text.
    pub accent: Color32,
    /// The accent for text and thin marks.
    pub accent_ink: Color32,
    pub on_accent: Color32,
    pub accent_tint: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub scrim: Color32,
    /// The selected fill and its text.
    pub selected: Color32,
    pub hover: Color32,
    pub net_telegram: Color32,
    pub net_signal: Color32,
    pub net_discord: Color32,
    pub net_whatsapp: Color32,
    /// The type roles' sizes (craft-libs docs/craft-framework.md, 5.2).
    pub caption: f32,
    pub label: f32,
    pub body: f32,
    pub title: f32,
    pub heading: f32,
    pub display: f32,
    /// Corner radii: a control's and a surface's.
    pub radius_control: f32,
    pub radius_surface: f32,
    /// A standard control's height.
    pub control: f32,
}

/// The network colours in a dark and in a light theme: each network's own, at text contrast on the theme's panels.
const NETWORKS_DARK: [Color32; 4] = [hex(0x7aa6d4), hex(0x6ea3ff), hex(0xa3a6ff), hex(0x58c486)];
const NETWORKS_LIGHT: [Color32; 4] = [hex(0x2b5278), hex(0x1c56c4), hex(0x4a4ec8), hex(0x17703c)];

impl Tokens {
    /// The tokens of the window's root theme, as craft-ui resolved them this frame.
    pub fn get(ctx: &egui::Context) -> Self {
        let env = Env::of_root(ctx);
        Self::from_resolved(&env.tokens.tokens().clone(), env.view().mode())
    }

    /// The values of `tokens`, resolved in `mode`.
    pub fn from_resolved(tokens: &craft_theme::Tokens, mode: Mode) -> Self {
        let c = &tokens.color;
        let ty = &tokens.typography;
        let dark = mode == Mode::Dark;
        let [net_telegram, net_signal, net_discord, net_whatsapp] = if dark { NETWORKS_DARK } else { NETWORKS_LIGHT };
        let accent = colour(c.accent.strong);
        Self {
            dark,
            bg: colour(c.surface.panel),
            sunken: colour(c.surface.background),
            raised: colour(c.surface.raised),
            ink: colour(c.text.normal),
            ink_strong: colour(c.text.normal),
            muted: colour(c.text.muted),
            faint: colour(c.text.faint),
            line: colour(c.line.divider),
            line_strong: colour(c.line.control_edge),
            accent,
            accent_ink: colour(c.accent.text),
            on_accent: colour(c.text.on_strong),
            accent_tint: colour(c.accent.soft),
            good: colour(c.status.good.fg),
            warn: colour(c.status.warn.fg),
            bad: colour(c.status.bad.fg),
            scrim: colour(c.surface.scrim),
            selected: colour(c.state.selected),
            hover: colour(c.state.hover),
            net_telegram,
            net_signal,
            net_discord,
            net_whatsapp,
            caption: ty.caption.size,
            label: ty.label.size,
            body: ty.body.size,
            title: ty.title.size,
            heading: ty.heading.size,
            display: ty.display.size,
            radius_control: tokens.shape.radius.control,
            radius_surface: tokens.shape.radius.surface,
            control: tokens.size.control.md,
        }
    }

    pub fn hairline(&self) -> Stroke {
        Stroke::new(1.0, self.line)
    }

    /// An accent colour a chat or network can take: one of the network colours.
    pub fn accent_colour(&self, accent: Accent) -> Color32 {
        match accent {
            Accent::Telegram => self.net_telegram,
            Accent::Signal => self.net_signal,
            Accent::WhatsApp => self.net_whatsapp,
            Accent::Discord => self.net_discord,
        }
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

/// Installs the fonts (craft-ui's font stack, with colour emoji where the build has them; the web build starts in
/// monochrome and fetches the colour font in the background). The look follows the theme each frame ([`apply`]).
pub fn install(ctx: &egui::Context) {
    crate::emoji::install(ctx);
}

/// Writes the theme craft-ui resolved for this frame into egui's style, for the parts PingCraft draws with egui. Both
/// of egui's theme styles get it, so the mode is craft-ui's alone. Cheap when nothing changed.
pub fn apply(ctx: &egui::Context) {
    let t = Tokens::get(ctx);
    let id = egui::Id::unique("pingcraft-applied-tokens");
    if ctx.data(|d| d.get_temp::<Tokens>(id)) == Some(t) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, t));
    ctx.all_styles_mut(|style| apply_to(style, &t));
}

fn apply_to(style: &mut egui::Style, t: &Tokens) {
    let v = &mut style.visuals;
    v.dark_mode = t.dark;
    v.panel_fill = t.bg;
    v.window_fill = t.raised;
    v.extreme_bg_color = t.sunken;
    v.faint_bg_color = t.sunken;
    v.override_text_color = Some(t.ink);
    v.hyperlink_color = t.accent_ink;
    v.selection.bg_fill = t.selected;
    v.selection.stroke = Stroke::new(1.0, t.ink_strong);
    v.window_stroke = Stroke::new(1.0, t.line);
    v.window_corner_radius = CornerRadius::from(t.radius_surface);
    v.menu_corner_radius = CornerRadius::from(t.radius_surface);
    v.error_fg_color = t.bad;
    v.warn_fg_color = t.warn;
    let radius = CornerRadius::from(t.radius_control);
    let w = &mut v.widgets;
    for (state, fill, stroke, text) in [
        (&mut w.noninteractive, t.bg, t.line, t.ink),
        (&mut w.inactive, t.sunken, t.line_strong, t.ink),
        (&mut w.hovered, t.hover, t.line_strong, t.ink_strong),
        (&mut w.active, t.hover, t.accent_ink, t.ink_strong),
        (&mut w.open, t.hover, t.line_strong, t.ink_strong),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, text);
        state.corner_radius = radius;
    }
    style.spacing.interact_size.y = t.control;
    for (text_style, size) in [
        (egui::TextStyle::Small, t.caption),
        (egui::TextStyle::Body, t.body),
        (egui::TextStyle::Button, t.label),
        (egui::TextStyle::Heading, t.heading),
        (egui::TextStyle::Monospace, t.body),
    ] {
        if let Some(font) = style.text_styles.get_mut(&text_style) {
            font.size = size;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tokens;
    use craft_theme::{Contrast, Mode};
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

    /// The network colours are data, not theme: the network tag's dot in them still stands out from craft-ui's default
    /// theme's panels and window background in both modes, at the 3:1 a cue that is not text needs (the tag's name is
    /// in the theme's own text colour, and says the network, so colour is not the only cue).
    #[test]
    fn network_colours_stand_out_on_the_themes_surfaces() {
        for mode in [Mode::Dark, Mode::Light] {
            let resolved = craft_ui::testing::style::tokens(mode, Contrast::Standard);
            let t = Tokens::from_resolved(resolved.tokens(), mode);
            for surface in [t.bg, t.sunken] {
                for net in [t.net_telegram, t.net_signal, t.net_discord, t.net_whatsapp] {
                    assert!(contrast(net, surface) >= 3.0, "{mode:?}: network colour {net:?} on {surface:?}");
                }
            }
        }
    }
}

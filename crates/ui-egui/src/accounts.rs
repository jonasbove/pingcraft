//! The ACCOUNTS tab: the networks a user can connect, their accounts, connecting one (the risk screen first, then
//! the bridge's login steps) and removing one. The design is `design/exports/connect-network.png` and
//! `connect-risk.png`; the words about each network's risk are PingCraft's own, from ADRs 0005 to 0008.

use egui::{Align, Color32, Layout, Margin, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};
use pingcraft_core::app::Command;
use pingcraft_core::bridge::{Answers, BridgeError, Display, FieldKind, LoginStep};
use pingcraft_core::{Network, RiskLevel};
use zeroize::Zeroizing;

use crate::i18n::{fmt, trn};
use crate::state::{Connect, NetworkEntry, State};
use crate::theme::Tokens;
use crate::tl;
use crate::view::Out;
use crate::widgets::{Kind, button, field, hud, network_name, note, problem};

/// The networks the screen offers, in the design's order. Discord waits for its bridge (ADR 0008).
pub const NETWORKS: &[&str] = &["telegram", "signal", "whatsapp", "discord"];

/// Whether PingCraft can connect this network yet. Discord's bridge is not on bridgev2 yet (ADR 0008).
fn offered(network: &Network) -> bool {
    network.id() != "discord"
}

pub fn risk_name(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => tl!("Low"),
        RiskLevel::Medium => tl!("Medium"),
        RiskLevel::MediumHigh => tl!("Medium-high"),
        RiskLevel::High => tl!("High"),
    }
}

/// How logging in works, for the network's card.
fn how_you_log_in(network: &Network) -> &'static str {
    match network.id() {
        "telegram" => tl!("Log in with your phone number and a code sent to Telegram, or scan a QR code."),
        "signal" => tl!("Link PingCraft as a device: scan a QR code with Signal on your phone."),
        "whatsapp" => tl!("Link PingCraft as a device: scan a QR code or type a pairing code."),
        "discord" => tl!("Comes with its bridge's next version. A bot login is low risk; logging in as yourself is high."),
        _ => tl!("Connected through a bridge on your server."),
    }
}

/// The risk screen's blocks for a network: a heading and a paragraph each, in PingCraft's words (ADRs 0005-0007).
fn risk_blocks(network: &Network) -> Vec<(&'static str, String)> {
    let name = network_name(network);
    let where_messages_are = (
        tl!("Where your messages are"),
        fmt(
            tl!(
                "The {network} bridge on your server can read your messages so it can pass them on. Keep your server's backups as safe as your phone."
            ),
            &[("network", &name)],
        ),
    );
    match network.id() {
        "telegram" => vec![
            (
                tl!("What could happen"),
                tl!("Telegram allows apps other than its own, so connecting through a bridge is within its rules. It can still limit an account it finds suspicious.").to_owned(),
            ),
            (tl!("What makes it more likely"), tl!("A new account is more likely to be flagged than one you have used for a while.").to_owned()),
            (
                tl!("How you log in"),
                tl!("With your phone number and a code Telegram sends to your Telegram app, and your two-step password if you set one; or by scanning a QR code.").to_owned(),
            ),
            where_messages_are,
        ],
        "signal" => vec![
            (
                tl!("What could happen"),
                tl!("Signal's terms don't say whether apps like this one are allowed. Signal also changes how it works from time to time, and the bridge can stop working until it catches up.").to_owned(),
            ),
            (
                tl!("How you log in"),
                tl!("PingCraft joins as a linked device, the way Signal's own desktop app does: you scan a QR code with Signal on your phone.").to_owned(),
            ),
            (
                where_messages_are.0,
                tl!("The Signal bridge on your server can read your messages so it can pass them on, and it keeps this device's Signal keys. Keep your server's backups as safe as your phone.").to_owned(),
            ),
        ],
        "whatsapp" => vec![
            (
                tl!("What could happen"),
                tl!("WhatsApp's terms don't allow unofficial apps, and WhatsApp does ban accounts it finds suspicious. PingCraft can't promise that yours won't be one of them.").to_owned(),
            ),
            (
                tl!("What makes it more likely"),
                tl!("A new account, a number from an internet phone service, and messaging people who haven't saved your number.").to_owned(),
            ),
            (
                tl!("Keep your phone online"),
                tl!("PingCraft joins as a linked device. If your phone stays offline for about 14 days, the link drops; the bridge warns you after 12.").to_owned(),
            ),
            where_messages_are,
        ],
        _ => vec![where_messages_are],
    }
}

/// The ACCOUNTS tab.
pub fn accounts(ui: &mut Ui, state: &mut State) {
    let t = Tokens::get(ui.ctx());
    egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(Margin::symmetric(24, 24))).show(ui, |ui| {
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            hud(ui, &format!("{} · {}", tl!("Accounts"), tl!("Connect a network")), t.muted);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(RichText::new(tl!("Connect a ")).size(40.0).strong().color(t.ink_strong));
                ui.label(RichText::new(tl!("network")).size(40.0).strong().color(t.accent_ink));
                ui.label(RichText::new(".").size(40.0).strong().color(t.ink_strong));
            });
            note(
                ui,
                tl!("Each network is connected through a bridge on your server. Choose one to see what connecting it means for your account, before you log in."),
            );
            ui.add_space(16.0);
            cards(ui, state);
            ui.add_space(8.0);
            note(
                ui,
                tl!("Risk level: how likely the network is to restrict or ban an account for being connected through a bridge. Each network's risk is explained before you connect it."),
            );
            if let Some(n) = state.accounts.notice.clone() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(n).size(13.0).color(t.ink));
                    if button(ui, tl!("Close"), Kind::Secondary, true).clicked() {
                        state.accounts.notice = None;
                    }
                });
            }
        });
    });
}

/// The four networks as cards that share hairlines.
fn cards(ui: &mut Ui, state: &mut State) {
    let t = Tokens::get(ui.ctx());
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.columns(NETWORKS.len(), |columns| {
        for (i, (id, ui)) in NETWORKS.iter().zip(columns.iter_mut()).enumerate() {
            let network = Network::from_id(id);
            let entry = state.accounts.networks.get(&network).cloned();
            egui::Frame::new().stroke(t.hairline()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(250.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                // The strip: number and risk level.
                egui::Frame::new().inner_margin(Margin::symmetric(20, 10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:02}", i + 1)).monospace().size(12.0).color(t.muted));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (word, color) = match network.risk_level() {
                                _ if !offered(&network) => (tl!("Later").to_owned(), t.muted),
                                Some(risk) => (risk_name(risk).to_owned(), risk_color(&t, risk)),
                                None => (tl!("Unknown").to_owned(), t.muted),
                            };
                            badge(ui, &word, color);
                        });
                    });
                });
                let r = ui.min_rect();
                ui.painter().hline(r.x_range(), r.bottom(), t.hairline());
                egui::Frame::new().inner_margin(Margin::symmetric(20, 14)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let name_color = if offered(&network) { t.ink_strong } else { t.muted };
                    ui.label(RichText::new(network_name(&network)).size(22.0).strong().color(name_color));
                    ui.add(egui::Label::new(RichText::new(how_you_log_in(&network)).size(13.0).color(t.muted)).wrap());
                    ui.add_space(12.0);
                    let (line, can_connect) = card_line(&network, entry.as_ref());
                    hud(ui, &line, t.muted);
                    let label = if offered(&network) { tl!("+ Connect") } else { tl!("Not yet") };
                    let enabled = can_connect && state.accounts.connect.is_none();
                    let response = button(ui, label, Kind::Secondary, enabled);
                    // Four cards, four buttons: each says which network it connects.
                    let spoken = if offered(&network) {
                        fmt(tl!("Connect {network}"), &[("network", &network_name(&network))])
                    } else {
                        fmt(tl!("{network}: not yet"), &[("network", &network_name(&network))])
                    };
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &spoken));
                    if response.clicked() {
                        state.accounts.connect = Some(Connect::Risk { network: network.clone(), understood: false });
                    }
                });
            });
        }
    });
}

/// The line above a card's button, and whether it can connect now.
fn card_line(network: &Network, entry: Option<&NetworkEntry>) -> (String, bool) {
    if !offered(network) {
        return (tl!("Not available yet").to_owned(), false);
    }
    match entry {
        None | Some(NetworkEntry::Loading) => (tl!("Asking the bridge…").to_owned(), false),
        Some(NetworkEntry::Ready(n)) if n.accounts.is_empty() => (tl!("No accounts yet").to_owned(), !n.flows.is_empty()),
        Some(NetworkEntry::Ready(n)) => (trn(n.accounts.len() as u64, "{n} account", "{n} accounts"), !n.flows.is_empty()),
        Some(NetworkEntry::Failed(BridgeError::NotAvailable)) => (tl!("Not on your server").to_owned(), false),
        Some(NetworkEntry::Failed(BridgeError::NotAuthorized)) => (tl!("The bridge didn't accept you").to_owned(), false),
        Some(NetworkEntry::Failed(_)) => (tl!("The bridge didn't answer").to_owned(), false),
    }
}

fn risk_color(t: &Tokens, risk: RiskLevel) -> Color32 {
    match risk {
        RiskLevel::Low => t.good,
        RiskLevel::Medium => t.warn,
        RiskLevel::MediumHigh | RiskLevel::High => t.bad,
    }
}

/// A small outlined word in mono capitals.
fn badge(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new().stroke(Stroke::new(1.0, color)).inner_margin(Margin::symmetric(6, 2)).show(ui, |ui| {
        ui.label(RichText::new(text.to_uppercase()).monospace().size(10.0).color(color));
    });
}

/// Connecting a network: the risk screen, then choosing how to log in, then the bridge's steps.
pub fn connect_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let Some(connect) = state.accounts.connect.take() else { return };
    let next = match connect {
        Connect::Risk { network, understood } => risk_sheet(ctx, state, network, understood, out),
        Connect::Choose { network } => choose_sheet(ctx, state, network, out),
        Connect::Starting { network } => waiting_sheet(ctx, network),
        Connect::Step { step, typed, sent } => step_sheet(ctx, step, typed, sent, out),
        Connect::Failed { network, error } => failed_sheet(ctx, network, error),
    };
    state.accounts.connect = next;
}

fn risk_sheet(ctx: &egui::Context, state: &State, network: Network, understood: bool, out: &mut Out) -> Option<Connect> {
    let name = network_name(&network);
    let risk = network.risk_level();
    let caption = format!("{} · {}", tl!("Risk level"), risk.map(risk_name).unwrap_or(tl!("Unknown")));
    let mut cancel = false;
    let mut go = false;
    let ticked = std::cell::Cell::new(understood);
    crate::widgets::sheet(
        ctx,
        "connect-risk",
        700.0,
        (&fmt(tl!("Before you connect {network}"), &[("network", &name)]), &caption),
        |ui| {
            let t = Tokens::get(ui.ctx());
            ui.horizontal(|ui| {
                hud(ui, tl!("Risk level"), t.ink);
                ui.spacing_mut().item_spacing.x = 0.0;
                for level in [RiskLevel::Low, RiskLevel::Medium, RiskLevel::MediumHigh, RiskLevel::High] {
                    let on = risk == Some(level);
                    let text = RichText::new(risk_name(level).to_uppercase()).monospace().size(11.0).color(if on { t.invert_fg } else { t.muted });
                    let cell = egui::Button::new(text)
                        .fill(if on { t.invert_bg } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, t.line_strong))
                        .selected(on);
                    // A scale to read, not a control: it does nothing when clicked, and says which level is this one.
                    let response = ui.add(cell.sense(Sense::hover()));
                    response.widget_info(|| {
                        let mut info = egui::WidgetInfo::labeled(egui::Role::ListItem, true, risk_name(level).to_uppercase());
                        info.selected = Some(on);
                        info
                    });
                }
            });
            for (heading, text) in risk_blocks(&network) {
                hud(ui, heading, t.ink);
                ui.add(egui::Label::new(RichText::new(text).size(13.5).color(t.muted)).wrap());
            }
            let sentence =
                fmt(tl!("I understand that {network} may restrict or ban this account, and I want to connect it anyway."), &[("network", &name)]);
            let mut on = ticked.get();
            ui.checkbox(&mut on, sentence);
            ticked.set(on);
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                go = button(ui, &fmt(tl!("→ Connect {network}"), &[("network", &name)]), Kind::Primary, ticked.get()).clicked();
            });
        },
    );
    let understood = ticked.get();
    if cancel {
        return None;
    }
    if !(go && understood) {
        return Some(Connect::Risk { network, understood });
    }
    let flows = match state.accounts.networks.get(&network) {
        Some(NetworkEntry::Ready(n)) => n.flows.clone(),
        _ => Vec::new(),
    };
    match flows.as_slice() {
        [] => Some(Connect::Failed { network, error: BridgeError::NotAvailable }),
        [only] => {
            out.push(Command::StartLogin { network: network.clone(), flow: only.id.clone() });
            Some(Connect::Starting { network })
        }
        _ => Some(Connect::Choose { network }),
    }
}

fn choose_sheet(ctx: &egui::Context, state: &State, network: Network, out: &mut Out) -> Option<Connect> {
    let name = network_name(&network);
    let flows = match state.accounts.networks.get(&network) {
        Some(NetworkEntry::Ready(n)) => n.flows.clone(),
        _ => Vec::new(),
    };
    let mut chosen = None;
    let mut cancel = false;
    crate::widgets::sheet(
        ctx,
        "connect-choose",
        560.0,
        (&fmt(tl!("Connect {network}"), &[("network", &name)]), tl!("How do you want to log in?")),
        |ui| {
            note(ui, tl!("These are the ways the bridge on your server offers, in its own words."));
            for flow in &flows {
                ui.horizontal(|ui| {
                    if button(ui, &flow.name, Kind::Secondary, true).clicked() {
                        chosen = Some(flow.id.clone());
                    }
                    note(ui, &flow.description);
                });
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
        },
    );
    if cancel {
        return None;
    }
    match chosen {
        Some(flow) => {
            out.push(Command::StartLogin { network: network.clone(), flow });
            Some(Connect::Starting { network })
        }
        None => Some(Connect::Choose { network }),
    }
}

fn waiting_sheet(ctx: &egui::Context, network: Network) -> Option<Connect> {
    let name = network_name(&network);
    crate::widgets::sheet(
        ctx,
        "connect-step",
        560.0,
        (&fmt(tl!("Connect {network}"), &[("network", &name)]), tl!("Asking the bridge")),
        |ui| {
            ui.label(RichText::new(tl!("Waiting for the bridge on your server…")).size(14.0));
        },
        |_| {},
    );
    Some(Connect::Starting { network })
}

fn step_sheet(
    ctx: &egui::Context,
    step: LoginStep,
    mut typed: std::collections::BTreeMap<String, Zeroizing<String>>,
    sent: bool,
    out: &mut Out,
) -> Option<Connect> {
    let (network, at) = match &step {
        LoginStep::Show { step: s, .. } | LoginStep::Ask { step: s, .. } | LoginStep::Unsupported { step: s, .. } => (s.network.clone(), s.clone()),
        // Done never stays open: the state closes the sheet.
        LoginStep::Done { .. } => return None,
    };
    let name = network_name(&network);
    let mut cancel = false;
    let submit = std::cell::Cell::new(false);
    let ready = match &step {
        LoginStep::Ask { fields, .. } => !sent && fields.iter().all(|f| f.optional || typed.get(&f.id).is_some_and(|v| !v.trim().is_empty())),
        _ => false,
    };
    crate::widgets::sheet(
        ctx,
        "connect-step",
        600.0,
        (&fmt(tl!("Connect {network}"), &[("network", &name)]), tl!("Logging in through your server's bridge")),
        |ui| match &step {
            LoginStep::Show { instructions, display, .. } => {
                if !instructions.is_empty() {
                    ui.add(egui::Label::new(RichText::new(instructions).size(14.0)).wrap());
                }
                match display {
                    Display::Qr(data) => qr_code(ui, data, &fmt(tl!("QR code to scan with {network}"), &[("network", &name)])),
                    Display::Code(code) | Display::Emoji(code) => {
                        ui.label(RichText::new(code).monospace().size(28.0).strong());
                    }
                    Display::Nothing => {}
                }
                note(ui, tl!("Waiting for you… This updates by itself."));
            }
            LoginStep::Ask { instructions, fields, .. } => {
                if !instructions.is_empty() {
                    ui.add(egui::Label::new(RichText::new(instructions).size(14.0)).wrap());
                }
                for f in fields {
                    let value = typed.entry(f.id.clone()).or_insert_with(|| Zeroizing::new(f.default.clone().unwrap_or_default()));
                    let hint = match f.kind {
                        FieldKind::PhoneNumber => "+45 12 34 56 78",
                        _ => "",
                    };
                    let response = field(ui, &f.name, value, f.kind.is_secret(), hint);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit.set(true);
                    }
                    if !f.description.is_empty() {
                        note(ui, &f.description);
                    }
                }
                if sent {
                    note(ui, tl!("Sent. Waiting for the bridge…"));
                }
            }
            LoginStep::Unsupported { kind, .. } => {
                problem(ui, &fmt(tl!("The bridge asks for a step PingCraft can't do yet ({kind})."), &[("kind", kind)]));
                note(ui, tl!("Cancel, and try another way to log in, or connect this network from another Matrix app for now."));
            }
            LoginStep::Done { .. } => {}
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            if let LoginStep::Ask { .. } = &step {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(ui, tl!("Continue"), Kind::Primary, ready).clicked() {
                        submit.set(true);
                    }
                });
            }
        },
    );
    if cancel {
        out.push(Command::CancelLogin(at));
        return None;
    }
    if submit.get() && ready {
        let mut answers = Answers::new();
        for (id, value) in &typed {
            answers.set(id, Zeroizing::new(value.trim().to_owned()));
        }
        out.push(Command::AnswerLogin { step: at, answers });
        return Some(Connect::Step { step, typed, sent: true });
    }
    Some(Connect::Step { step, typed, sent })
}

fn failed_sheet(ctx: &egui::Context, network: Network, error: BridgeError) -> Option<Connect> {
    let name = network_name(&network);
    let mut close = false;
    crate::widgets::sheet(
        ctx,
        "connect-step",
        560.0,
        (&fmt(tl!("Connect {network}"), &[("network", &name)]), tl!("Not connected")),
        |ui| {
            problem(ui, &failure(&error));
        },
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = button(ui, tl!("Close"), Kind::Primary, true).clicked();
            });
        },
    );
    if close { None } else { Some(Connect::Failed { network, error }) }
}

/// Why connecting did not work, in plain words.
pub fn failure(error: &BridgeError) -> String {
    match error {
        BridgeError::NotAvailable => tl!("This network's bridge is not on your server. Your server's admin can add it.").to_owned(),
        BridgeError::NotAuthorized => {
            tl!("The bridge could not confirm who you are with your server. Ask your server's admin to check the bridge's setup.").to_owned()
        }
        BridgeError::Refused(why) => fmt(tl!("The bridge said: {why}"), &[("why", why)]),
        BridgeError::LoginGone => tl!("The login ended before it was finished, perhaps because it took too long. Start again.").to_owned(),
        BridgeError::Unreachable(why) => fmt(tl!("The bridge could not be reached: {why}"), &[("why", why)]),
    }
}

/// A QR code, black on white whatever the theme, with a quiet zone, so a phone can read it.
fn qr_code(ui: &mut Ui, data: &str, label: &str) {
    let Ok(code) = qrcode::QrCode::new(data.as_bytes()) else {
        problem(ui, tl!("This QR code could not be drawn."));
        return;
    };
    const QUIET: usize = 4;
    let width = code.width();
    let modules = width + 2 * QUIET;
    let size = 240.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label));
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    let cell = size / modules as f32;
    for (i, color) in code.to_colors().iter().enumerate() {
        if *color == qrcode::Color::Dark {
            let (x, y) = ((i % width + QUIET) as f32, (i / width + QUIET) as f32);
            let min = rect.min + Vec2::new(x * cell, y * cell);
            painter.rect_filled(egui::Rect::from_min_size(min, Vec2::splat(cell + 0.5)), 0.0, Color32::BLACK);
        }
    }
}

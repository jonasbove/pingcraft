//! The open chat: its header, the messages, the composer (with the reply and edit bars), selecting messages, and
//! the sheets for a message's details, forwarding and deleting.

use egui::{Align, Color32, Layout, Margin, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};
use pingcraft_core::app::Command;
use pingcraft_core::{Chat, Network};

use crate::i18n::fmt;
use crate::message::Act;
use crate::state::{Forwarding, OpenChat};
use crate::theme::Tokens;
use crate::time::day_label;
use crate::tl;
use crate::view::Out;
use crate::widgets::{Kind, avatar, button, hud, network_name, network_tag, note};

pub fn chat_view(ui: &mut Ui, open: &mut OpenChat, chat: Option<&Chat>, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    let name = chat.map(|c| c.name.clone()).unwrap_or_default();
    // The header.
    egui::Panel::top("chat-header").exact_size(64.0).frame(egui::Frame::new().inner_margin(Margin::symmetric(24, 0))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            avatar(ui, &name, 36.0);
            ui.vertical(|ui| {
                // Centre the name and tag on the avatar: a vertical block in a centred row starts at the top.
                let name_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(16.0)));
                let tag_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::monospace(10.0))) + 2.0;
                let height = if chat.is_some() { name_h + ui.spacing().item_spacing.y + tag_h } else { name_h };
                ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
                ui.label(RichText::new(&name).size(16.0).strong().color(t.ink_strong));
                if let Some(chat) = chat {
                    network_tag(ui, &chat.network);
                }
            });
            if let Some(chat) = chat {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (text, color) = match (chat.encrypted, chat.network.is_matrix()) {
                        (true, true) => (tl!("End-to-end encrypted"), t.good),
                        (true, false) => (tl!("Encrypted · via your bridge"), t.muted),
                        (false, _) => (tl!("Not encrypted"), t.warn),
                    };
                    if chat.encrypted {
                        egui::Frame::new().stroke(t.hairline()).inner_margin(Margin::symmetric(10, 5)).show(ui, |ui| {
                            ui.label(RichText::new(text).size(12.5).color(color));
                        });
                    } else {
                        // Not encrypted: a button that offers to turn it on.
                        let badge = egui::Button::new(RichText::new(text).size(12.5).color(color))
                            .stroke(t.hairline())
                            .corner_radius(egui::CornerRadius::ZERO);
                        let response = crate::widgets::animated(ui, badge, true, Color32::TRANSPARENT, t.line);
                        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Not encrypted: turn on encryption")));
                        if response.clicked() {
                            open.encrypt_sheet = true;
                        }
                    }
                });
            }
        });
    });
    if open.encrypt_sheet {
        encrypt_sheet(ui.ctx(), open, &name, out);
    }
    // The composer, with the reply or edit bar over it; while selecting, what can be done with the selection. Hidden
    // under a sheet, so the sheet's buttons are the only ones with their names.
    if open.selected.is_some() {
        if open.forwarding.is_none() && open.deleting.is_empty() {
            selection_bar(ui, open);
        }
    } else {
        composer(ui, open, &name, chat.map(|c| &c.network), out);
    }
    // The timeline, newest at the bottom.
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(Margin::symmetric(24, 16))).show(ui, |ui| {
        ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            if !open.at_start && !open.messages.is_empty() && button(ui, tl!("Load older messages"), Kind::Secondary, true).clicked() {
                out.push(Command::LoadOlder(open.id.clone()));
            }
            let mut last_day = String::new();
            let mut acts = Vec::new();
            for message in &open.messages {
                let day = day_label(message.at);
                if day != last_day {
                    divider(ui, &day);
                    last_day = day;
                }
                let ticked = open.selected.as_ref().map(|s| s.contains(&message.id));
                let (rect, act) = crate::message::message_view(ui, message, &open.id, ticked, out);
                if open.jump_to.as_ref() == Some(&message.id) {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                }
                acts.extend(act);
                ui.add_space(14.0);
            }
            open.jump_to = None;
            for act in acts {
                apply(open, act, out);
            }
        });
    });
    let deleting: Vec<_> = open.messages.iter().filter(|m| open.deleting.contains(&m.id)).cloned().collect();
    if !deleting.is_empty() && crate::message::delete_sheet(ui.ctx(), &deleting, &open.id, &name, out) {
        // Deleting from a selection ends it, whether or not the user went ahead.
        if open.deleting.len() > 1 || open.selected.is_some() {
            open.selected = None;
        }
        open.deleting.clear();
    }
    if open.details.is_some() {
        details_sheet(ui.ctx(), open, chat);
    }
}

/// Does what a message asked for.
fn apply(open: &mut OpenChat, act: Act, out: &mut Out) {
    match act {
        Act::Reply(id) => {
            open.stop_editing();
            open.replying = Some(id);
        }
        Act::Edit(id) => open.edit(id),
        Act::Delete(id) => open.deleting = vec![id],
        Act::Jump(id) => open.jump_to = Some(id),
        Act::LoadOlder => {
            if !open.at_start {
                out.push(Command::LoadOlder(open.id.clone()));
            }
        }
        Act::Forward(id) => open.forwarding = Some(Forwarding { messages: vec![id], ..Forwarding::default() }),
        Act::Select(id) => {
            open.stop_editing();
            open.replying = None;
            open.selected = Some(vec![id]);
        }
        Act::Toggle(id) => open.toggle(id),
        Act::Details(id) => {
            out.push(Command::Details { chat: open.id.clone(), message: id.clone() });
            open.details = Some((id, None));
        }
    }
}

/// Instead of the composer while selecting: how many are selected, and COPY, FORWARD, DELETE and CANCEL.
fn selection_bar(ui: &mut Ui, open: &mut OpenChat) {
    let selection = open.selection();
    let count = selection.len();
    let deletable = count > 0 && selection.iter().all(|m| m.can.delete);
    let forwardable = selection.iter().any(|m| matches!(m.body, pingcraft_core::MessageBody::Text(_)));
    let text = crate::message::copy_lines(&selection);
    let ids: Vec<_> = selection.iter().map(|m| m.id.clone()).collect();
    egui::Panel::bottom("selection").exact_size(74.0).frame(egui::Frame::new().inner_margin(Margin::symmetric(24, 20))).show(ui, |ui| {
        ui.horizontal(|ui| {
            let t = Tokens::get(ui.ctx());
            ui.label(RichText::new(fmt(tl!("{n} selected"), &[("n", &count.to_string())])).monospace().size(11.0).color(t.ink));
            if count > 0 && !deletable {
                note(ui, tl!("Only your own messages can be deleted."));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, tl!("Cancel"), Kind::Secondary, true).clicked() {
                    open.selected = None;
                }
                if button(ui, tl!("Delete"), Kind::Secondary, deletable).clicked() {
                    open.deleting = ids.clone();
                }
                if button(ui, tl!("Forward"), Kind::Secondary, forwardable).clicked() {
                    open.forwarding = Some(Forwarding { messages: ids.clone(), ..Forwarding::default() });
                }
                if button(ui, tl!("Copy"), Kind::Secondary, count > 0).clicked() {
                    ui.ctx().copy_text(text);
                }
            });
        });
    });
    let line = ui.min_rect();
    ui.painter().hline(line.x_range(), line.bottom(), Tokens::get(ui.ctx()).hairline());
}

/// The words for how a message is encrypted, as the chat header says it.
fn encryption_words(encrypted: bool, network: Option<&Network>) -> &'static str {
    match (encrypted, network.is_none_or(Network::is_matrix)) {
        (true, true) => tl!("End-to-end encrypted"),
        (true, false) => tl!("Encrypted between your server and you; the bridge reads it to pass it on"),
        (false, _) => tl!("Not encrypted"),
    }
}

/// A message's details: when it was sent, the network, encryption, who read it, and its earlier versions.
fn details_sheet(ctx: &egui::Context, open: &mut OpenChat, chat: Option<&Chat>) {
    let Some((id, details)) = open.details.clone() else { return };
    let Some(message) = open.message(&id).cloned() else { return };
    let mut close = false;
    let network = chat.map(|c| &c.network);
    let caption = format!("{} · {}", if message.own { tl!("You") } else { &message.sender }, chat.map(|c| c.name.as_str()).unwrap_or_default());
    crate::widgets::sheet(
        ctx,
        // A new id once the details are in: egui keeps a sheet where it first was, and the loaded one is taller.
        if details.is_some() { "message-details" } else { "message-details-loading" },
        620.0,
        (tl!("Message details"), &caption.to_uppercase()),
        |ui| {
            let t = Tokens::get(ui.ctx());
            ui.label(RichText::new(crate::view::preview_text(&message.body)).size(13.0).color(t.muted));
            let Some(d) = &details else {
                note(ui, tl!("Loading…"));
                return;
            };
            let fact = |ui: &mut Ui, label: &str, value: &str| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 16.0), egui::Sense::hover());
                    ui.put(rect, egui::Label::new(RichText::new(label.to_uppercase()).monospace().size(11.0).color(t.muted)));
                    ui.label(RichText::new(value).size(12.5));
                });
            };
            fact(ui, tl!("Sent"), &crate::time::full(d.sent));
            if let Some(n) = network {
                let via = if n.is_matrix() { network_name(n) } else { fmt(tl!("{network}, through your bridge"), &[("network", &network_name(n))]) };
                fact(ui, tl!("Network"), &via);
            }
            fact(ui, tl!("Encryption"), encryption_words(d.encrypted, network));
            let read = if d.read_by.is_empty() {
                tl!("No one yet, or the network doesn't say").to_owned()
            } else {
                d.read_by
                    .iter()
                    .map(|(who, at)| match at {
                        Some(at) => format!("{who} {}", crate::time::clock(*at)),
                        None => who.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            fact(ui, tl!("Read by"), &read);
            if d.versions.len() > 1 {
                hud(ui, tl!("Edit history"), t.muted);
                for (at, body) in &d.versions {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(at.map(crate::time::clock).unwrap_or_default()).monospace().size(12.0).color(t.muted));
                        ui.label(RichText::new(crate::view::preview_text(body)).size(12.5));
                    });
                }
            }
            note(ui, tl!("Read receipts and edits show only where the network and its bridge carry them."));
        },
        |ui| {
            if let pingcraft_core::MessageBody::Text(text) = &message.body
                && button(ui, tl!("Copy text"), Kind::Secondary, true).clicked()
            {
                ui.ctx().copy_text(text.clone());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                close = button(ui, tl!("Close"), Kind::Primary, true).clicked();
            });
        },
    );
    if close {
        open.details = None;
    }
}

/// The message field and SEND, with a bar over them while replying to a message or editing one of the user's.
fn composer(ui: &mut Ui, open: &mut OpenChat, name: &str, network: Option<&pingcraft_core::Network>, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    let editing = open.editing.as_ref().map(|(id, _)| id.clone());
    let replying = open.replying.as_ref().and_then(|id| open.message(id)).map(|m| (m.sender.clone(), m.own, crate::view::preview_text(&m.body)));
    let mut send = false;
    egui::Panel::bottom("composer").frame(egui::Frame::new().inner_margin(Margin::ZERO)).show(ui, |ui| {
        let bar = |ui: &mut Ui, title: &str, line: &str, cancel: &str| -> bool {
            let mut cancelled = false;
            egui::Frame::new().fill(t.sunken).inner_margin(Margin::symmetric(24, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).monospace().size(11.0).color(t.accent_ink));
                        ui.add(egui::Label::new(RichText::new(line).size(12.5).color(t.muted)).truncate());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let x = ui.add(
                            egui::Button::new(RichText::new("×").size(13.0))
                                .stroke(Stroke::new(1.0, t.line_strong))
                                .corner_radius(egui::CornerRadius::ZERO)
                                .min_size(Vec2::splat(30.0)),
                        );
                        x.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, cancel));
                        cancelled = x.clicked();
                    });
                });
            });
            cancelled
        };
        if editing.is_some() {
            let what = match network {
                Some(n) if !n.is_matrix() => fmt(
                    tl!("Edited messages say “edited”. Whether the edit reaches {network} depends on its bridge."),
                    &[("network", &network_name(n))],
                ),
                _ => tl!("Edited messages say “edited”.").to_owned(),
            };
            if bar(ui, tl!("Editing your message"), &what, tl!("Cancel editing")) {
                open.stop_editing();
            }
        } else if let Some((sender, own, line)) = &replying {
            let sender = if *own { tl!("yourself").to_owned() } else { sender.clone() };
            if bar(ui, &fmt(tl!("Replying to {sender}"), &[("sender", &sender)]), line, tl!("Cancel reply")) {
                open.replying = None;
            }
        }
        egui::Frame::new().inner_margin(Margin::symmetric(24, 20)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = fmt(tl!("Message {chat}"), &[("chat", name)]);
                let width = ui.available_width() - 100.0;
                let edit = egui::TextEdit::singleline(&mut open.draft).hint_text(&label).desired_width(width).margin(Margin::symmetric(10, 8));
                let response = ui.add(edit);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    send = true;
                    response.request_focus();
                }
                let verb = if editing.is_some() { tl!("Save") } else { tl!("Send") };
                send |= button(ui, verb, Kind::Primary, !open.draft.trim().is_empty()).clicked();
            });
        });
    });
    if !send || open.draft.trim().is_empty() {
        return;
    }
    let text = std::mem::take(&mut open.draft).trim().to_owned();
    let chat = open.id.clone();
    out.push(match (open.editing.take(), open.replying.take()) {
        (Some((message, before)), _) => {
            open.draft = before;
            Command::Edit { chat, message, text }
        }
        (None, Some(to)) => Command::Reply { chat, to, text },
        (None, None) => Command::SendText { chat, text },
    });
}

fn divider(ui: &mut Ui, label: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 160.0).max(20.0) / 2.0;
        let (left, _) = ui.allocate_exact_size(Vec2::new(w, 12.0), Sense::hover());
        ui.painter().hline(left.x_range(), left.center().y, t.hairline());
        hud(ui, label, t.muted);
        let (right, _) = ui.allocate_exact_size(Vec2::new(w, 12.0), Sense::hover());
        ui.painter().hline(right.x_range(), right.center().y, t.hairline());
    });
    ui.add_space(6.0);
}

/// Offers to turn on end-to-end encryption in a chat that has none.
fn encrypt_sheet(ctx: &egui::Context, open: &mut OpenChat, name: &str, out: &mut Out) {
    let mut close = false;
    let mut turn_on = false;
    crate::widgets::sheet(
        ctx,
        "encrypt-chat",
        520.0,
        (tl!("Turn on encryption"), &name.to_uppercase()),
        |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.label(
                RichText::new(tl!("New messages in this chat will be end-to-end encrypted: only the people in it can read them, not the server."))
                    .size(14.0),
            );
            note(
                ui,
                tl!(
                    "Encryption can't be turned off again. Messages sent before stay unencrypted. Everyone in the chat needs an app that supports encryption to read new messages."
                ),
            );
            note(ui, tl!("Turning it on needs permission to change the chat's settings. If you don't have it, ask someone who does."));
        },
        |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                turn_on = button(ui, tl!("Turn on encryption"), Kind::Primary, true).clicked();
                close = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            });
        },
    );
    if turn_on {
        out.push(Command::EnableEncryption(open.id.clone()));
    }
    if turn_on || close {
        open.encrypt_sheet = false;
    }
}

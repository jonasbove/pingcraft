//! The open chat: its header, the messages, the composer (with the reply and edit bars), selecting messages, and
//! the sheets for a message's details, forwarding and deleting.

use egui::{Align, Color32, Layout, Margin, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};
use pingcraft_core::app::Command;
use pingcraft_core::{Alerts, Chat, Mute, Network};

use crate::emoji;
use crate::i18n::fmt;
use crate::message::{Act, Cx};
use crate::state::{Forwarding, OpenChat};
use crate::theme::Tokens;
use crate::time::day_label;
use crate::tl;
use crate::view::Out;
use crate::widgets::{Kind, avatar, button, hud, network_name, note};

/// The open chat. Returns whether the user left it (marking it unread).
pub fn chat_view(ui: &mut Ui, open: &mut OpenChat, chat: Option<&Chat>, network: Alerts, out: &mut Out) -> bool {
    let t = Tokens::get(ui.ctx());
    let name = chat.map(|c| c.name.clone()).unwrap_or_default();
    let mut leave = false;
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
                ui.label(emoji::text(&name, 16.0, t.ink_strong));
                if let Some(chat) = chat {
                    crate::widgets::network_tag_in(ui, &chat.network, chat.alerts.over(network).accent);
                }
            });
            if let Some(chat) = chat {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    leave = chat_menu(ui, chat, network, out);
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
    if leave {
        return true;
    }
    if open.encrypt_sheet {
        encrypt_sheet(ui.ctx(), open, &name, out);
    }
    if !open.pinned.is_empty() {
        pinned_bar(ui, open, out);
    }
    // The composer, with the reply or edit bar over it; while selecting, what can be done with the selection. Hidden
    // under a sheet, so the sheet's buttons are the only ones with their names.
    if open.selected.is_some() {
        if open.forwarding.is_none() && open.deleting.is_empty() {
            selection_bar(ui, open);
        }
    } else {
        composer(ui, open, &name, chat, out);
    }
    // The timeline, newest at the bottom.
    egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(Margin::symmetric(24, 16))).show(ui, |ui| {
        let area = ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]);
        let shown = area.show(ui, |ui| {
            if !open.at_start && !open.messages.is_empty() && button(ui, tl!("Load older messages"), Kind::Secondary, true).clicked() {
                out.push(Command::LoadOlder(open.id.clone()));
            }
            let mut last_day = String::new();
            let mut acts = Vec::new();
            let pinned: Vec<_> = open.pinned.iter().filter_map(|q| q.message.clone()).collect();
            for message in &open.messages {
                let day = day_label(message.at);
                if day != last_day {
                    divider(ui, &day);
                    last_day = day;
                }
                let ticked = open.selected.as_ref().map(|s| s.contains(&message.id));
                let cx = Cx { chat: &open.id, pinned: &pinned };
                let (rect, act) = crate::message::message_view(ui, message, &cx, ticked, out);
                if open.jump_to.as_ref() == Some(&message.id) {
                    ui.scroll_to_rect(rect, Some(Align::Center));
                }
                acts.extend(act);
                ui.add_space(14.0);
            }
            open.jump_to = None;
            if std::mem::take(&mut open.to_bottom) {
                ui.scroll_to_cursor(Some(Align::BOTTOM));
            }
            for act in acts {
                apply(open, act, out);
            }
        });
        let bottom = shown.state.clamped_offset().y + shown.inner_rect.height();
        open.at_bottom = bottom >= shown.content_size.y - 24.0;
        if !open.at_bottom {
            // Over the bottom right of the timeline.
            let size = Vec2::new(150.0, 34.0);
            let rect = egui::Rect::from_min_size(shown.inner_rect.right_bottom() - size - Vec2::new(8.0, 8.0), size);
            let jump = ui.put(
                rect,
                egui::Button::new(RichText::new(tl!("Jump to latest").to_uppercase()).monospace().size(11.0)).fill(Tokens::get(ui.ctx()).bg),
            );
            jump.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Jump to latest")));
            if jump.clicked() {
                open.to_bottom = true;
            }
        }
    });
    let deleting: Vec<_> = open.messages.iter().filter(|m| open.deleting.contains(&m.id)).cloned().collect();
    let cx = Cx { chat: &open.id, pinned: &[] };
    if !deleting.is_empty() && crate::message::delete_sheet(ui.ctx(), &deleting, &cx, &name, out) {
        // Deleting from a selection ends it, whether or not the user went ahead.
        if open.deleting.len() > 1 || open.selected.is_some() {
            open.selected = None;
        }
        open.deleting.clear();
    }
    if open.details.is_some() {
        details_sheet(ui.ctx(), open, chat);
    }
    if open.reacting.is_some() {
        reaction_picker(ui.ctx(), open, out);
    }
    false
}

/// The chat's "…" menu: pin it to the top of the inbox, mark it unread, archive it, mute it. Returns whether the
/// user left the chat (marking it unread, or archiving it).
fn chat_menu(ui: &mut Ui, chat: &Chat, network: Alerts, out: &mut Out) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut leave = false;
    let more = ui.add(
        egui::Button::new(RichText::new("…").size(14.0))
            .stroke(Stroke::new(1.0, t.line_strong))
            .corner_radius(egui::CornerRadius::ZERO)
            .min_size(Vec2::splat(34.0)),
    );
    more.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Chat actions")));
    egui::Popup::menu(&more).show(|ui| {
        ui.set_min_width(260.0);
        let id = chat.id.clone();
        let pin = if chat.pinned { tl!("Unpin from the top of the inbox") } else { tl!("Pin to the top of the inbox") };
        if ui.button(pin).clicked() {
            out.push(Command::PinChat { chat: id.clone(), pin: !chat.pinned });
        }
        if ui.button(tl!("Mark as unread")).clicked() {
            out.push(Command::CloseChat);
            out.push(Command::MarkUnread(id.clone()));
            leave = true;
        }
        if chat.is_archived() {
            if ui.button(tl!("Move back to the inbox")).clicked() {
                out.push(Command::Archive { chat: id.clone(), archive: false });
            }
        } else if ui.button(tl!("Archive")).clicked() {
            out.push(Command::CloseChat);
            out.push(Command::Archive { chat: id.clone(), archive: true });
            leave = true;
        }
        note(ui, tl!("An archived chat comes back to the inbox when a new message arrives, unless it is muted."));
        ui.separator();
        hud(ui, tl!("Notifications"), t.muted);
        match chat.muted {
            Some(mute) => {
                let until = match mute {
                    Mute::Until(at) => fmt(tl!("Muted until {time}"), &[("time", &crate::time::full(at))]),
                    Mute::UntilUnmuted => tl!("Muted until you turn it back on").to_owned(),
                };
                note(ui, &until);
                if ui.button(tl!("Turn notifications back on")).clicked() {
                    out.push(Command::Mute { chat: id.clone(), mute: None });
                }
            }
            None => {
                for (label, hours) in [(tl!("Mute for an hour"), 1), (tl!("Mute for a day"), 24), (tl!("Mute for a week"), 24 * 7)] {
                    if ui.button(label).clicked() {
                        out.push(Command::Mute { chat: id.clone(), mute: Some(Mute::Until(crate::time::hours_from_now(hours))) });
                    }
                }
                if ui.button(tl!("Mute until you turn it back on")).clicked() {
                    out.push(Command::Mute { chat: id.clone(), mute: Some(Mute::UntilUnmuted) });
                }
            }
        }
        note(ui, tl!("A muted chat still counts as unread. PingCraft turns notifications back on at the time you chose while it is running."));
        ui.separator();
        hud(ui, tl!("Sound and colour"), t.muted);
        if let Some(alerts) = crate::widgets::alerts_controls(ui, chat.alerts, chat.alerts.over(network), "", tl!("the network's")) {
            out.push(Command::ChatAlerts { chat: id.clone(), alerts });
        }
        note(ui, tl!("Your system's own notification sound. Kept with your account, in PingCraft only: the network is not told."));
    });
    leave
}

/// The pinned messages, one at a time, newest first, under the chat header. Clicking one goes to it.
fn pinned_bar(ui: &mut Ui, open: &mut OpenChat, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    let n = open.pinned.len();
    let i = open.pin_shown.min(n.saturating_sub(1));
    let Some(quote) = open.pinned.get(n - 1 - i).cloned() else { return };
    let text = crate::view::preview_text(&quote.body);
    let sender = if quote.sender.is_empty() { tl!("A message") } else { quote.sender.as_str() };
    let count = fmt(tl!("Pinned {i} of {n}"), &[("i", &(i + 1).to_string()), ("n", &n.to_string())]);
    egui::Panel::top("pinned").exact_size(44.0).frame(egui::Frame::new().fill(t.sunken).inner_margin(Margin::symmetric(24, 0))).show(ui, |ui| {
        let bar = ui.max_rect();
        ui.painter().vline(bar.left() + 1.0, bar.y_range(), Stroke::new(2.0, t.accent_ink));
        ui.horizontal_centered(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if n > 1 {
                    let next = ui.add(egui::Button::new(RichText::new("›").size(16.0)).frame(false));
                    next.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Next pinned message")));
                    if next.clicked() {
                        open.pin_shown = (i + 1) % n;
                    }
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let line = format!("{sender}: {text}");
                    let shown = ui.add(
                        egui::Label::new(emoji::text(&format!("{}   {line}", count.to_uppercase()), 12.5, t.ink)).truncate().sense(Sense::click()),
                    );
                    let label = format!("{count}: {line}");
                    shown.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
                    if shown.clicked() {
                        match &quote.message {
                            Some(id) => open.jump_to = Some(id.clone()),
                            None if !open.at_start => out.push(Command::LoadOlder(open.id.clone())),
                            None => {}
                        }
                    }
                });
            });
        });
    });
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
        Act::ChooseReaction(id) => open.reacting = Some(id),
    }
}

/// The narrowest the selection bar's note is shown at, cut short.
const MIN_NOTE_WIDTH: f32 = 80.0;

/// Instead of the composer while selecting: how many are selected, and COPY, FORWARD, DELETE and CANCEL.
fn selection_bar(ui: &mut Ui, open: &mut OpenChat) {
    let selection = open.selection();
    let count = selection.len();
    let deletable = count > 0 && selection.iter().all(|m| m.can.delete);
    let forwardable = selection.iter().any(|m| matches!(m.body, pingcraft_core::MessageBody::Text(_)));
    let text = crate::message::copy_lines(&selection);
    let ids: Vec<_> = selection.iter().map(|m| m.id.clone()).collect();
    egui::Panel::bottom("selection").exact_size(74.0).frame(egui::Frame::new().inner_margin(Margin::symmetric(24, 20))).show(ui, |ui| {
        // The buttons first, from the right, so the count and the note get only the space left and never lie under
        // them.
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
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let t = Tokens::get(ui.ctx());
                ui.label(RichText::new(fmt(tl!("{n} selected"), &[("n", &count.to_string())])).monospace().size(11.0).color(t.ink));
                // Cut short when it does not fit (the whole sentence on hover, and as its accessible name); left out
                // when hardly a word would show.
                if count > 0 && !deletable && ui.available_width() >= MIN_NOTE_WIDTH {
                    ui.add(egui::Label::new(RichText::new(tl!("Only your own messages can be deleted.")).size(12.5).color(t.muted)).truncate());
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
fn composer(ui: &mut Ui, open: &mut OpenChat, name: &str, chat: Option<&Chat>, out: &mut Out) {
    let network = chat.map(|c| &c.network);
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
                        ui.add(egui::Label::new(emoji::text(line, 12.5, t.muted)).truncate());
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
        if let Some(line) = typing_line(&open.typing) {
            egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 6, bottom: 0 }).show(ui, |ui| {
                ui.label(emoji::text(&line, 12.5, t.muted));
            });
        }
        let edit_id = egui::Id::unique(("pingcraft-composer", &open.id.0));
        shortcode_suggestions(ui, open, edit_id);
        egui::Frame::new().inner_margin(Margin::symmetric(24, 20)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = fmt(tl!("Message {chat}"), &[("chat", name)]);
                if let Some(e) = emoji_button(ui, edit_id) {
                    crate::picker::remember(ui.ctx(), &e);
                    let at = cursor(ui.ctx(), edit_id).unwrap_or_else(|| open.draft.chars().count());
                    put(ui.ctx(), edit_id, &mut open.draft, (at, at), &e);
                }
                let width = ui.available_width() - 100.0;
                // Emoji in the draft are laid out like those in messages, in colour.
                let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let format = egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ui.visuals().text_color());
                    let mut job = emoji::job(text.as_str(), format);
                    job.wrap.max_width = wrap_width;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let output = egui::TextEdit::singleline(&mut open.draft)
                    .id(edit_id)
                    .hint_text(&label)
                    .desired_width(width)
                    .margin(Margin::symmetric(10, 8))
                    .layouter(&mut layouter)
                    .show(ui);
                let response = output.response.response;
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
                if response.changed() {
                    // A finished `:name:` becomes its emoji.
                    if let Some(at) = output.cursor_range.map(|r| r.primary.index.0)
                        && let Some((text, at)) = crate::picker::complete_shortcode(&open.draft, at, crate::picker::tone(ui.ctx()))
                    {
                        open.draft = text;
                        set_cursor(ui.ctx(), edit_id, at);
                    }
                    tell_typing(open, ui.input(|i| i.time), out);
                }
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
    let chat_kept = chat;
    let chat = open.id.clone();
    if open.typing_told.take().is_some() {
        out.push(Command::Typing { chat: chat.clone(), typing: false });
    }
    out.push(match (open.editing.take(), open.replying.take()) {
        (Some((message, before)), _) => {
            open.draft = before;
            Command::Edit { chat, message, text }
        }
        (None, Some(to)) => Command::Reply { chat, to, text },
        (None, None) => Command::SendText { chat, text },
    });
    // The draft kept in the store has been sent: forget it, or it would come back.
    if let Some(c) = chat_kept.filter(|c| c.draft.is_some()) {
        out.push(Command::SaveDraft { chat: c.id.clone(), text: String::new(), draft_for: None });
    }
}

/// The emoji picker for a reaction, over the chat. Escape or a click outside closes it.
fn reaction_picker(ctx: &egui::Context, open: &mut OpenChat, out: &mut Out) {
    let Some(id) = open.reacting.clone() else { return };
    let t = Tokens::get(ctx);
    let frame = egui::Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.line_strong)).inner_margin(Margin::same(16));
    let modal = egui::Modal::new(egui::Id::unique("pingcraft-reaction-picker")).frame(frame).backdrop_color(t.scrim).show(ctx, |ui| {
        hud(ui, tl!("React"), t.muted);
        crate::picker::picker(ui, egui::Id::unique("pingcraft-reaction-picker").with("picker"))
    });
    if let Some(key) = modal.inner {
        crate::picker::remember(ctx, &key);
        out.push(Command::React { chat: open.id.clone(), message: id, key });
        open.reacting = None;
    } else if modal.should_close() {
        open.reacting = None;
    }
}

/// The composer's emoji button and its picker. Returns the emoji chosen.
fn emoji_button(ui: &mut Ui, edit_id: egui::Id) -> Option<String> {
    let t = Tokens::get(ui.ctx());
    let pick = ui.add(
        egui::Button::new(emoji::symbol("😀", 16.0, Color32::PLACEHOLDER))
            .stroke(Stroke::new(1.0, t.line_strong))
            .corner_radius(egui::CornerRadius::ZERO)
            .min_size(Vec2::splat(34.0)),
    );
    pick.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Emoji")));
    let mut chosen = None;
    egui::Popup::menu(&pick).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        chosen = crate::picker::picker(ui, edit_id.with("picker"));
        if chosen.is_some() {
            ui.close();
        }
    });
    chosen
}

/// Where the composer's text cursor is (a char index), if it has one.
fn cursor(ctx: &egui::Context, edit_id: egui::Id) -> Option<usize> {
    egui::text_edit::TextEditState::load(ctx, edit_id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0)
}

/// Puts the composer's text cursor at a char index and focuses it.
fn set_cursor(ctx: &egui::Context, edit_id: egui::Id, at: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, edit_id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at))));
    state.store(ctx, edit_id);
    ctx.memory_mut(|m| m.request_focus(edit_id));
}

/// Replaces the chars `from..to` of the draft with `text`, and puts the cursor after it.
fn put(ctx: &egui::Context, edit_id: egui::Id, draft: &mut String, (from, to): (usize, usize), text: &str) {
    let chars: Vec<char> = draft.chars().collect();
    let (from, to) = (from.min(chars.len()), to.min(chars.len()));
    let mut out: String = chars[..from].iter().collect();
    out.push_str(text);
    let at = out.chars().count();
    out.extend(&chars[to.max(from)..]);
    *draft = out;
    set_cursor(ctx, edit_id, at);
}

/// While a `:name` is being typed, the shortcodes it may become, over the composer.
fn shortcode_suggestions(ui: &mut Ui, open: &mut OpenChat, edit_id: egui::Id) {
    let Some(at) = cursor(ui.ctx(), edit_id) else { return };
    let Some((colon, word)) = crate::picker::partial_shortcode(&open.draft, at) else { return };
    let found = crate::picker::suggestions(&word, crate::picker::tone(ui.ctx()));
    if found.is_empty() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().inner_margin(Margin { left: 24, right: 24, top: 6, bottom: 0 }).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (code, e) in found {
                let chip = egui::Button::new(emoji::text(&format!("{e} :{code}:"), 12.5, t.ink))
                    .stroke(t.hairline())
                    .corner_radius(egui::CornerRadius::ZERO);
                let response = ui.add(chip);
                let label = fmt(tl!("Insert {emoji} :{code}:"), &[("emoji", &e), ("code", code)]);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
                if response.clicked() {
                    crate::picker::remember(ui.ctx(), &e);
                    put(ui.ctx(), edit_id, &mut open.draft, (colon, at), &e);
                }
            }
        });
    });
}

/// How often the chat is told again that the user is still typing (servers drop it after about 30 seconds).
const TYPING_AGAIN: f64 = 20.0;

/// Tells the chat the user is typing (again, now and then, while they go on), or stopped (the field is empty).
fn tell_typing(open: &mut OpenChat, now: f64, out: &mut Out) {
    let typing = !open.draft.trim().is_empty() && open.editing.is_none();
    match (typing, open.typing_told) {
        (true, Some(at)) if now - at < TYPING_AGAIN => {}
        (true, _) => {
            open.typing_told = Some(now);
            out.push(Command::Typing { chat: open.id.clone(), typing: true });
        }
        (false, Some(_)) => {
            open.typing_told = None;
            out.push(Command::Typing { chat: open.id.clone(), typing: false });
        }
        (false, None) => {}
    }
}

/// Who is typing, in words.
fn typing_line(names: &[String]) -> Option<String> {
    match names {
        [] => None,
        [one] => Some(fmt(tl!("{name} is typing…"), &[("name", one)])),
        [a, b] => Some(fmt(tl!("{a} and {b} are typing…"), &[("a", a), ("b", b)])),
        _ => Some(tl!("Several people are typing…").to_owned()),
    }
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

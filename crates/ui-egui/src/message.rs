//! One message in the open chat, and what can be done with it: reactions, replying, editing, deleting, copying,
//! forwarding, selecting and its details.

use egui::{Align, Color32, Layout, Margin, Rect, RichText, Sense, Stroke, Ui};
use pingcraft_core::app::Command;
use pingcraft_core::{ChatId, Message, MessageBody, MessageId, Quote, Reaction, SendState};

use crate::emoji;
use crate::i18n::fmt;
use crate::theme::Tokens;
use crate::tl;
use crate::view::{Out, preview_text, unsupported};
use crate::widgets::{Kind, avatar, button, note};

/// The quick reactions in a message's menu. Sent as written, with the variation selector the networks expect.
pub const QUICK_REACTIONS: [&str; 6] = ["👍", "❤\u{fe0f}", "😂", "😮", "😢", "🙏"];

/// A reaction key in words for accessible names: variation selectors dropped, and at most a few characters, since a
/// key can be any text another app sent.
pub fn shown_key(key: &str) -> String {
    let mut shown: String = key.chars().filter(|c| !matches!(c, '\u{fe00}'..='\u{fe0f}')).take(8).collect();
    if key.chars().filter(|c| !matches!(c, '\u{fe00}'..='\u{fe0f}')).count() > 8 {
        shown.push('…');
    }
    shown
}

/// A reaction key as drawn: as sent (so `❤️` and keycaps stay emoji), cut after a few grapheme clusters and a
/// bounded number of characters, since a key can be any text another app sent.
pub fn drawn_key(key: &str) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let mut drawn = String::new();
    let mut chars = 0;
    for (i, cluster) in key.graphemes(true).enumerate() {
        chars += cluster.chars().count();
        if i == 4 || chars > 32 {
            drawn.push('…');
            break;
        }
        drawn.push_str(cluster);
    }
    drawn
}

/// What a message needs to know about its chat.
pub struct Cx<'a> {
    pub chat: &'a ChatId,
    /// The chat's pinned messages.
    pub pinned: &'a [MessageId],
}

/// What a message asked the open chat to do, beyond sending a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Reply(MessageId),
    Edit(MessageId),
    Delete(MessageId),
    /// Scroll to this message (a quote was clicked).
    Jump(MessageId),
    /// The quoted message is not loaded: load older ones.
    LoadOlder,
    Forward(MessageId),
    /// Start selecting, with this message ticked.
    Select(MessageId),
    /// Tick or untick while selecting.
    Toggle(MessageId),
    Details(MessageId),
}

/// The most names a reaction's accessible name lists before "and N more".
const MAX_NAMES: usize = 10;

/// How the sender is called in the interface.
fn who(message: &Message) -> String {
    if message.own { tl!("You").to_owned() } else { message.sender.clone() }
}

/// A message's text in one short line, for labels.
fn snippet(body: &MessageBody) -> String {
    let line = preview_text(body);
    let mut short: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        short.push('…');
    }
    short
}

/// One message. `ticked` is whether it is selected, while selecting. Returns its rectangle (to scroll to it) and
/// what it asked for.
pub fn message_view(ui: &mut Ui, message: &Message, cx: &Cx<'_>, ticked: Option<bool>, out: &mut Out) -> (Rect, Option<Act>) {
    let Some(on) = ticked else { return message_only(ui, message, cx, out) };
    let t = Tokens::get(ui.ctx());
    let mut act = None;
    let row = egui::Frame::new().fill(if on { t.sunken } else { Color32::TRANSPARENT }).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let mut value = on;
            let label = fmt(tl!("Select: {sender}, {text}"), &[("sender", &who(message)), ("text", &snippet(&message.body))]);
            let response = ui.add(egui::Checkbox::without_text(&mut value));
            response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &label));
            if response.clicked() {
                act = Some(Act::Toggle(message.id.clone()));
            }
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                let (_, inner) = message_only(ui, message, cx, out);
                act = act.take().or(inner);
            });
        });
    });
    (row.response.rect, act)
}

fn message_only(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out) -> (Rect, Option<Act>) {
    // Right-clicking anywhere on the message opens its menu. The area is the message's place in the last frame,
    // claimed before its contents so the buttons inside stay on top of it.
    let area_id = ui.scope_id().with(("message-area", &message.id.0));
    let last: Option<Rect> = ui.ctx().data(|d| d.get_temp(area_id));
    let mut act = None;
    if let Some(rect) = last {
        let area = ui.interact(rect, area_id, Sense::click());
        area.context_menu(|ui| {
            if let Some(a) = menu(ui, message, cx, out) {
                act = Some(a);
            }
        });
    }
    let rect = if message.own { own(ui, message, cx, out, &mut act) } else { theirs(ui, message, cx, out, &mut act) };
    ui.ctx().data_mut(|d| d.insert_temp(area_id, rect));
    (rect, act)
}

fn theirs(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, act: &mut Option<Act>) -> Rect {
    ui.horizontal_top(|ui| {
        avatar(ui, &message.sender, 28.0);
        ui.add_space(4.0);
        ui.vertical(|ui| contents(ui, message, cx, out, act));
    })
    .response
    .rect
}

fn own(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, act: &mut Option<Act>) -> Rect {
    let t = Tokens::get(ui.ctx());
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        let rule = if matches!(message.state, SendState::Failed(_)) { t.bad } else { t.accent_ink };
        let frame = egui::Frame::new().fill(t.accent_tint).inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
            ui.set_max_width(460.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                contents(ui, message, cx, out, act);
                if let SendState::Failed(_) = message.state {
                    ui.horizontal(|ui| {
                        note(ui, tl!("The server didn't take it."));
                        if button(ui, tl!("Retry"), Kind::Primary, true).clicked() {
                            out.push(Command::Retry { chat: cx.chat.clone(), message: message.id.clone() });
                        }
                    });
                }
            });
        });
        let r = frame.response.rect;
        ui.painter().vline(r.right() - 1.0, r.y_range(), Stroke::new(2.0, rule));
        r
    })
    .inner
}

/// The head, the quote, the body and the reactions.
fn contents(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, act: &mut Option<Act>) {
    head(ui, message, cx, out, act);
    if let Some(quote) = &message.reply_to
        && let Some(a) = quote_view(ui, quote, &message.id)
    {
        *act = Some(a);
    }
    // The text is selectable, so it takes right-clicks itself: it opens the same menu.
    body(ui, &message.body).context_menu(|ui| {
        if let Some(a) = menu(ui, message, cx, out) {
            *act = Some(a);
        }
    });
    if !message.reactions.is_empty() {
        reactions(ui, message, cx, out);
    }
}

fn head(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, act: &mut Option<Act>) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(emoji::text(&who(message), 13.0, t.ink_strong));
        ui.label(RichText::new(crate::time::clock(message.at)).monospace().size(12.0).color(t.muted));
        if message.edited {
            ui.label(RichText::new(tl!("edited")).monospace().size(12.0).color(t.muted));
        }
        if message.own {
            ui.add_space(24.0);
            let (state, color) = match &message.state {
                SendState::Sent => (tl!("Sent"), t.muted),
                SendState::Sending => (tl!("Sending…"), t.muted),
                SendState::Failed(_) => (tl!("Not sent"), t.bad),
            };
            ui.label(RichText::new(state.to_uppercase()).monospace().size(11.0).color(color));
        }
        if message.body != MessageBody::Deleted {
            more(ui, message, cx, out, act);
        }
    });
}

/// The "…" button that opens the message's menu.
fn more(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, act: &mut Option<Act>) {
    let t = Tokens::get(ui.ctx());
    let label = fmt(tl!("Message actions: {sender}, {text}"), &[("sender", &who(message)), ("text", &snippet(&message.body))]);
    let response = ui.add(egui::Button::new(RichText::new("…").size(14.0).color(t.muted)).frame(false).min_size(egui::vec2(24.0, 18.0)));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    egui::Popup::menu(&response).show(|ui| {
        if let Some(a) = menu(ui, message, cx, out) {
            *act = Some(a);
        }
    });
}

/// The message's menu: quick reactions, then what the user may do with it.
fn menu(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out) -> Option<Act> {
    ui.set_min_width(220.0);
    let mut act = None;
    if message.can.react {
        ui.horizontal(|ui| {
            for key in QUICK_REACTIONS {
                let shown = shown_key(key);
                let mut chip = egui::Button::new(emoji::text(&drawn_key(key), 18.0, Color32::PLACEHOLDER)).frame(false);
                if message.my_reaction(key) {
                    chip = chip.selected(true);
                }
                let response = ui.add(chip);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, fmt(tl!("React with {emoji}"), &[("emoji", &shown)])));
                if response.clicked() {
                    out.push(Command::React { chat: cx.chat.clone(), message: message.id.clone(), key: key.to_owned() });
                    ui.close();
                }
            }
        });
        ui.separator();
    }
    let item = |ui: &mut Ui, label: &str, enabled: bool| enabled && ui.button(label).clicked();
    if item(ui, tl!("Reply"), message.can.reply) {
        act = Some(Act::Reply(message.id.clone()));
    }
    let text = match &message.body {
        MessageBody::Text(text) => Some(text),
        _ => None,
    };
    if let Some(text) = text
        && item(ui, tl!("Copy text"), true)
    {
        ui.ctx().copy_text(text.clone());
        ui.close();
    }
    if item(ui, tl!("Forward"), text.is_some() && message.state == SendState::Sent) {
        act = Some(Act::Forward(message.id.clone()));
    }
    let pinned = cx.pinned.contains(&message.id);
    if item(ui, if pinned { tl!("Unpin") } else { tl!("Pin in this chat") }, pinned || message.can.react) {
        out.push(Command::PinMessage { chat: cx.chat.clone(), message: message.id.clone(), pin: !pinned });
        ui.close();
    }
    if item(ui, tl!("Select"), true) {
        act = Some(Act::Select(message.id.clone()));
    }
    if item(ui, tl!("Details"), true) {
        act = Some(Act::Details(message.id.clone()));
    }
    if item(ui, tl!("Edit"), message.can.edit && text.is_some()) {
        act = Some(Act::Edit(message.id.clone()));
    }
    if message.can.delete {
        let t = Tokens::get(ui.ctx());
        if ui.button(RichText::new(tl!("Delete")).color(t.bad)).clicked() {
            act = Some(Act::Delete(message.id.clone()));
        }
    }
    if act.is_some() {
        ui.close();
    }
    act
}

/// The quoted original above a reply. Clicking it goes to the original.
fn quote_view(ui: &mut Ui, quote: &Quote, reply: &MessageId) -> Option<Act> {
    let t = Tokens::get(ui.ctx());
    let text = snippet(&quote.body);
    let sender = if quote.sender.is_empty() { tl!("a message") } else { quote.sender.as_str() };
    let inner = egui::Frame::new().inner_margin(Margin { left: 10, right: 0, top: 2, bottom: 2 }).show(ui, |ui| {
        ui.set_max_width(420.0);
        ui.label(emoji::text(sender, 12.0, t.accent_ink));
        ui.add(egui::Label::new(emoji::text(&text, 12.5, t.muted)).truncate());
    });
    let rect = inner.response.rect;
    ui.painter().vline(rect.left() + 1.0, rect.y_range(), Stroke::new(2.0, t.line_strong));
    let response = ui.interact(rect, ui.scope_id().with(("quote", &reply.0)), Sense::click());
    let label = fmt(tl!("Reply to {sender}: {text}"), &[("sender", sender), ("text", &text)]);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    if !response.clicked() {
        return None;
    }
    Some(match &quote.message {
        Some(id) => Act::Jump(id.clone()),
        None => Act::LoadOlder,
    })
}

/// The reactions under a message: one chip per emoji with its count, in the accent when the user is among them.
fn reactions(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out) {
    ui.horizontal_wrapped(|ui| {
        for reaction in &message.reactions {
            if chip(ui, reaction, message.can.react).clicked() {
                out.push(Command::React { chat: cx.chat.clone(), message: message.id.clone(), key: reaction.key.clone() });
            }
        }
    });
}

fn chip(ui: &mut Ui, reaction: &Reaction, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let shown = shown_key(&reaction.key);
    let count = reaction.by.len().max(1);
    let text = emoji::text(&format!("{} {count}", drawn_key(&reaction.key)), 13.0, if reaction.mine { t.accent_ink } else { t.ink });
    let stroke = Stroke::new(1.0, if reaction.mine { t.accent_ink } else { t.line_strong });
    let fill = if reaction.mine { t.accent_tint } else { Color32::TRANSPARENT };
    let response = ui.add_enabled(enabled, egui::Button::new(text).fill(fill).stroke(stroke).corner_radius(egui::CornerRadius::ZERO));
    let mut names = reaction.by.iter().take(MAX_NAMES).cloned().collect::<Vec<_>>().join(", ");
    if reaction.by.len() > MAX_NAMES {
        names = fmt(tl!("{names} and {n} more"), &[("names", &names), ("n", &(reaction.by.len() - MAX_NAMES).to_string())]);
    }
    let label = fmt(tl!("Reaction {emoji} from {names}"), &[("emoji", &shown), ("names", &names)]);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Button, enabled, &label);
        info.selected = Some(reaction.mine);
        info
    });
    response.on_hover_text(names)
}

fn body(ui: &mut Ui, body: &MessageBody) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    match body {
        MessageBody::Text(text) => ui.add(egui::Label::new(emoji::text(text, 14.0, Color32::PLACEHOLDER)).wrap()),
        MessageBody::WaitingForKey => {
            egui::Frame::new()
                .fill(t.sunken)
                .stroke(t.hairline())
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_max_width(500.0);
                    ui.label(RichText::new(tl!("Waiting for the key to this message")).size(14.0));
                    note(ui, tl!("It was sent before this device could read it. It opens by itself once one of your other devices, or your key backup, shares the key."));
                })
                .response
        }
        MessageBody::Deleted => ui.label(RichText::new(tl!("Message deleted")).italics().size(13.0).color(t.muted)),
        MessageBody::Loading => ui.label(RichText::new("…").size(14.0).color(t.muted)),
        MessageBody::Unsupported(kind) => ui.label(RichText::new(unsupported(kind)).italics().size(14.0).color(t.muted)),
    }
}

/// Asks before deleting some of the user's messages.
pub fn delete_sheet(ctx: &egui::Context, messages: &[Message], cx: &Cx<'_>, chat_name: &str, out: &mut Out) -> bool {
    let mut close = false;
    let title = match messages {
        [_] => tl!("Delete this message?").to_owned(),
        _ => fmt(tl!("Delete {n} messages?"), &[("n", &messages.len().to_string())]),
    };
    crate::widgets::sheet(
        ctx,
        "delete-message",
        560.0,
        (&title, &chat_name.to_uppercase()),
        |ui| {
            for message in messages.iter().take(5) {
                ui.label(emoji::text(&snippet(&message.body), 13.0, Color32::PLACEHOLDER));
            }
            ui.label(RichText::new(tl!("They are deleted for everyone in this chat, and a “Message deleted” line stays in their place.")).size(14.0));
            note(ui, tl!("PingCraft asks the network's bridge to delete them there too. Whether they disappear there depends on the network."));
        },
        |ui| {
            if button(ui, tl!("Cancel"), Kind::Secondary, true).clicked() {
                close = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, tl!("Delete"), Kind::Danger, true).clicked() {
                    for message in messages {
                        out.push(Command::Delete { chat: cx.chat.clone(), message: message.id.clone() });
                    }
                    close = true;
                }
            });
        },
    );
    close
}

/// The selected messages as text, for the clipboard: "sender, time: text", one per line.
pub fn copy_lines(messages: &[&Message]) -> String {
    messages
        .iter()
        .map(|m| fmt(tl!("{sender}, {time}: {text}"), &[("sender", &who(m)), ("time", &crate::time::clock(m.at)), ("text", &full_text(&m.body))]))
        .collect::<Vec<_>>()
        .join("\n")
}

fn full_text(body: &MessageBody) -> String {
    match body {
        MessageBody::Text(text) => text.clone(),
        other => preview_text(other),
    }
}

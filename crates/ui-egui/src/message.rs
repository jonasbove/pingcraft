//! One message in the open chat, and what can be done with it: reactions, replying, editing, deleting, copying,
//! forwarding, selecting and its details.

use egui::{Align, Color32, Layout, Margin, Rect, RichText, Sense, Stroke, Ui};
use pingcraft_core::app::Command;
use pingcraft_core::{ChatId, EmojiPresentation, Message, MessageBody, MessageId, Quote, Reaction, SendState};

use crate::emoji;
use crate::i18n::fmt;
use crate::theme::Tokens;
use crate::tl;
use crate::view::{Out, preview_text, unsupported};
use crate::widgets::{Kind, avatar, button, note};
use craft_ui::contract::{Show as _, ShowContainer as _, Trigger};
use craft_ui::display::Divider;
use craft_ui::menu::{Menu, MenuItem};

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
    /// How the chat's network's app draws symbols that are text by default ([`crate::emoji::is_emoji`]).
    pub emoji: EmojiPresentation,
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
    /// Choose a reaction in the emoji picker.
    ChooseReaction(MessageId),
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
    let area = last.map(|rect| ui.interact(rect, area_id, Sense::click()));
    ui.data_mut(|d| match area {
        Some(area) => {
            d.insert_temp(more_id(message).with("area"), area);
        }
        None => {
            d.remove::<egui::Response>(more_id(message).with("area"));
        }
    });
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
    head(ui, message, cx);
    if let Some(quote) = &message.reply_to
        && let Some(a) = quote_view(ui, quote, &message.id, cx.emoji)
    {
        *act = Some(a);
    }
    // The text is selectable, so it takes right-clicks itself: it opens the same menu.
    let text = body(ui, &message.body, cx.emoji);
    if let Some(more) = more_button(ui, message)
        && let Some(a) = menu(ui, message, cx, out, &more, &text)
    {
        *act = Some(a);
    }
    if !message.reactions.is_empty() {
        reactions(ui, message, cx, out);
    }
}

fn head(ui: &mut Ui, message: &Message, cx: &Cx<'_>) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(emoji::text(&who(message), t.body, t.ink_strong, cx.emoji));
        ui.label(RichText::new(crate::time::clock(message.at)).size(t.label).color(t.muted));
        if message.edited {
            ui.label(RichText::new(tl!("edited")).size(t.label).color(t.muted));
        }
        if message.own {
            ui.add_space(24.0);
            let (state, color) = match &message.state {
                SendState::Sent => (tl!("Sent"), t.muted),
                SendState::Sending => (tl!("Sending…"), t.muted),
                SendState::Failed(_) => (tl!("Not sent"), t.bad),
            };
            ui.label(RichText::new(state).size(t.caption).color(color));
        }
        if message.body != MessageBody::Deleted {
            let more = crate::widgets::menu_button(ui, crate::widgets::icon::MORE, &actions_label(message));
            ui.data_mut(|d| d.insert_temp(more_id(message), more.response));
        }
    });
}

/// The accessible name of a message's menu and of the button that opens it.
fn actions_label(message: &Message) -> String {
    fmt(tl!("Message actions: {sender}, {text}"), &[("sender", &who(message)), ("text", &snippet(&message.body))])
}

fn more_id(message: &Message) -> egui::Id {
    egui::Id::unique("pingcraft-message-more").with(&message.id.0)
}

/// The message's "more" button drawn in its head this pass, which its menu hangs from.
fn more_button(ui: &Ui, message: &Message) -> Option<egui::Response> {
    (message.body != MessageBody::Deleted).then(|| ui.data(|d| d.get_temp::<egui::Response>(more_id(message)))).flatten()
}

/// The message's menu, craft-ui's `Menu`: it opens from the "more" button (click), or a right-click on the message or
/// its text. Quick reactions (recently used first) and the picker, then what the user may do with it. Each row's `&`
/// marks the letter craft-ui's menu runs it from while it is open (R reply, E more reactions, C copy text, F forward,
/// P pin, S select, I details, T edit, D delete); not shown to work yet, so no row shows it as a shortcut (PLAN.md,
/// "Messages and chats" 12).
fn menu(ui: &mut Ui, message: &Message, cx: &Cx<'_>, out: &mut Out, more: &egui::Response, text_response: &egui::Response) -> Option<Act> {
    let mut act = None;
    let text = match &message.body {
        MessageBody::Text(text) => Some(text.clone()),
        _ => None,
    };
    let pinned = cx.pinned.contains(&message.id);
    let mut menu = Menu::new(actions_label(message)).opened_by(more, Trigger::Click).opened_by(text_response, Trigger::Context);
    let area = ui.data(|d| d.get_temp::<egui::Response>(more_id(message).with("area")));
    if let Some(area) = &area {
        menu = menu.opened_by(area, Trigger::Context);
    }
    menu.show(ui, |ui, menu_cx| {
        let mut close = false;
        if message.can.react {
            ui.horizontal(|ui| {
                for key in crate::picker::quick_reactions(ui.ctx()) {
                    let key = key.as_str();
                    let shown = shown_key(key);
                    let mut chip = egui::Button::new(emoji::symbol(&drawn_key(key), 18.0, Color32::PLACEHOLDER)).frame(false);
                    if message.my_reaction(key) {
                        chip = chip.selected(true);
                    }
                    let response = ui.add(chip);
                    response
                        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, fmt(tl!("React with {emoji}"), &[("emoji", &shown)])));
                    if response.clicked() {
                        crate::picker::remember(ui.ctx(), key);
                        out.push(Command::React { chat: cx.chat.clone(), message: message.id.clone(), key: key.to_owned() });
                        close = true;
                    }
                }
            });
            if row(ui, tl!("Mor&e reactions…"), true) {
                act = Some(Act::ChooseReaction(message.id.clone()));
            }
            Divider::horizontal().show(ui);
        }
        if row(ui, tl!("&Reply"), message.can.reply) {
            act = Some(Act::Reply(message.id.clone()));
        }
        if let Some(text) = &text
            && row(ui, tl!("&Copy text"), true)
        {
            ui.ctx().copy_text(text.clone());
        }
        if row(ui, tl!("&Forward"), text.is_some() && message.state == SendState::Sent) {
            act = Some(Act::Forward(message.id.clone()));
        }
        if row(ui, if pinned { tl!("Un&pin") } else { tl!("&Pin in this chat") }, pinned || message.can.react) {
            out.push(Command::PinMessage { chat: cx.chat.clone(), message: message.id.clone(), pin: !pinned });
        }
        if row(ui, tl!("&Select"), true) {
            act = Some(Act::Select(message.id.clone()));
        }
        if row(ui, tl!("Deta&ils"), true) {
            act = Some(Act::Details(message.id.clone()));
        }
        if row(ui, tl!("Edi&t"), message.can.edit && text.is_some()) {
            act = Some(Act::Edit(message.id.clone()));
        }
        if message.can.delete && row(ui, tl!("&Delete"), true) {
            act = Some(Act::Delete(message.id.clone()));
        }
        // A quick reaction is an egui button, not a row, so it closes the menu itself.
        if close {
            menu_cx.request_close();
        }
    });
    act
}

/// One row of the message's menu: `label` with its `&` mnemonic; left out when the message does not allow it. Whether
/// it ran.
fn row(ui: &mut Ui, label: &str, allowed: bool) -> bool {
    allowed && MenuItem::new(label.to_owned()).show(ui).ran
}

/// The quoted original above a reply. Clicking it goes to the original.
fn quote_view(ui: &mut Ui, quote: &Quote, reply: &MessageId, presentation: EmojiPresentation) -> Option<Act> {
    let t = Tokens::get(ui.ctx());
    let text = snippet(&quote.body);
    let sender = if quote.sender.is_empty() { tl!("a message") } else { quote.sender.as_str() };
    let inner = egui::Frame::new().inner_margin(Margin { left: 10, right: 0, top: 2, bottom: 2 }).show(ui, |ui| {
        ui.set_max_width(420.0);
        ui.label(emoji::text(sender, t.label, t.accent_ink, presentation));
        ui.add(egui::Label::new(emoji::text(&text, t.label, t.muted, presentation)).truncate());
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
            if chip(ui, reaction, message.can.react, cx.emoji).clicked() {
                out.push(Command::React { chat: cx.chat.clone(), message: message.id.clone(), key: reaction.key.clone() });
            }
        }
    });
}

fn chip(ui: &mut Ui, reaction: &Reaction, enabled: bool, presentation: EmojiPresentation) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let shown = shown_key(&reaction.key);
    let count = reaction.by.len().max(1);
    let text = emoji::text(&format!("{} {count}", drawn_key(&reaction.key)), t.body, if reaction.mine { t.accent_ink } else { t.ink }, presentation);
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

fn body(ui: &mut Ui, body: &MessageBody, presentation: EmojiPresentation) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    match body {
        MessageBody::Text(text) => ui.add(egui::Label::new(emoji::body(text, t.body, Color32::PLACEHOLDER, presentation)).wrap()),
        MessageBody::WaitingForKey => {
            egui::Frame::new()
                .fill(t.sunken)
                .stroke(t.hairline())
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_max_width(500.0);
                    ui.label(RichText::new(tl!("Waiting for the key to this message")).size(t.body));
                    note(ui, tl!("It was sent before this device could read it. It opens by itself once one of your other devices, or your key backup, shares the key."));
                })
                .response
        }
        MessageBody::Deleted => ui.label(RichText::new(tl!("Message deleted")).italics().size(t.body).color(t.muted)),
        MessageBody::Loading => ui.label(RichText::new("…").size(t.body).color(t.muted)),
        MessageBody::Unsupported(kind) => ui.label(RichText::new(unsupported(kind)).italics().size(t.body).color(t.muted)),
    }
}

/// Asks before deleting some of the user's messages.
pub fn delete_sheet(ctx: &egui::Context, messages: &[Message], cx: &Cx<'_>, chat_name: &str, out: &mut Out) -> bool {
    let mut close = false;
    let title = match messages {
        [_] => tl!("Delete this message?").to_owned(),
        _ => fmt(tl!("Delete {n} messages?"), &[("n", &messages.len().to_string())]),
    };
    let dismissed = crate::widgets::sheet(
        ctx,
        "delete-message",
        560.0,
        (&title, chat_name),
        |ui| {
            for message in messages.iter().take(5) {
                ui.label(emoji::text(&snippet(&message.body), Tokens::get(ui.ctx()).body, Color32::PLACEHOLDER, cx.emoji));
            }
            ui.label(
                RichText::new(tl!("They are deleted for everyone in this chat, and a “Message deleted” line stays in their place."))
                    .size(Tokens::get(ui.ctx()).body),
            );
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
    close |= dismissed;
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

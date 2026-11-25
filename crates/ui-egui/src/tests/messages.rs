//! What can be done with a message: reactions, replies, editing and deleting.

use egui::accesskit::Toggled;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pingcraft_core::app::{Command, Event};
use pingcraft_core::{Abilities, ChatId, MessageBody, MessageId, Quote, Reaction, SendState};

use super::{Fake, message, signed_in, type_into};
use crate::PingCraftApp;

const ALL: Abilities = Abilities { reply: true, react: true, edit: true, delete: true };
const THEIRS: Abilities = Abilities { reply: true, react: true, edit: false, delete: false };

fn text(m: &str) -> MessageBody {
    MessageBody::Text(m.into())
}

/// Book club open, with a message from Lars (reacted to), one of the user's own (edited), a deleted one and a reply.
pub(super) fn book_club(fake: &Fake) -> Harness<'static, PingCraftApp> {
    let mut h = signed_in(fake);
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.take();
    let mut lars = message("1", "Lars Holm", false, text("Next meeting on Thursday?"), SendState::Sent);
    lars.can = THEIRS;
    lars.reactions = vec![Reaction { key: "👍".into(), by: vec!["Lars Holm".into(), "Ana".into()], mine: true }];
    let mut mine = message("2", "@Ana:example.org", true, text("Thursday works."), SendState::Sent);
    mine.can = ALL;
    mine.edited = true;
    let gone = message("3", "Lars Holm", false, MessageBody::Deleted, SendState::Sent);
    let mut reply = message("4", "Mei Tanaka", false, text("I'll bring bread"), SendState::Sent);
    reply.can = THEIRS;
    reply.reply_to = Some(Quote { message: Some(MessageId("1".into())), sender: "Lars Holm".into(), body: text("Next meeting on Thursday?") });
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: vec![lars, mine, gone, reply] }]);
    h.run();
    h
}

fn actions(h: &mut Harness<'static, PingCraftApp>, sender: &str, text: &str) {
    h.get_by_label(&format!("Message actions: {sender}, {text}")).click();
    h.run();
}

#[test]
fn edited_deleted_reactions_and_replies_are_shown() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    h.get_by_label("edited");
    h.get_by_label("Message deleted");
    let chip = h.get_by_label("Reaction 👍 from Lars Holm, Ana");
    assert_eq!(chip.accesskit_node().toggled(), Some(Toggled::True), "the user is one of those who reacted");
    h.get_by_label("Reply to Lars Holm: Next meeting on Thursday?");
    // Clicking the chip takes the user's reaction back.
    h.get_by_label("Reaction 👍 from Lars Holm, Ana").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::React { message, key, .. }] if message.0 == "1" && key == "👍"), "{sent:?}");
}

#[test]
fn replying_quotes_the_message_and_sends_a_reply() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    actions(&mut h, "Lars Holm", "Next meeting on Thursday?");
    h.get_by_label("Reply").click();
    h.run();
    h.get_by_label("Replying to Lars Holm");
    type_into(&mut h, "Message Book club", "Yes, at mine");
    h.get_by_label("SEND").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::Reply { to, text, .. }] if to.0 == "1" && text == "Yes, at mine"), "{sent:?}");
    assert!(h.query_by_label("Replying to Lars Holm").is_none(), "the reply bar closes once sent");
}

#[test]
fn a_quick_reaction_is_sent_from_the_menu() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    actions(&mut h, "Lars Holm", "Next meeting on Thursday?");
    h.get_by_label("React with 😂").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::React { message, key, .. }] if message.0 == "1" && key == "😂"), "{sent:?}");
}

#[test]
fn only_your_own_messages_can_be_edited_or_deleted() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    actions(&mut h, "Lars Holm", "Next meeting on Thursday?");
    assert!(h.query_by_label("Edit").is_none() && h.query_by_label("Delete").is_none());
    h.key_press(egui::Key::Escape);
    h.run();
    actions(&mut h, "You", "Thursday works.");
    h.get_by_label("Edit");
    h.get_by_label("Delete");
}

#[test]
fn editing_puts_the_text_in_the_composer_and_saves_it() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    actions(&mut h, "You", "Thursday works.");
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Editing your message");
    type_into(&mut h, "Message Book club", " See you!");
    h.get_by_label("SAVE").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::Edit { message, text, .. }] if message.0 == "2" && text == "Thursday works. See you!"), "{sent:?}");
}

#[test]
fn deleting_asks_first() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    actions(&mut h, "You", "Thursday works.");
    h.get_by_label("Delete").click();
    h.run();
    h.get_by_label("Delete this message?");
    h.get_by_label("CANCEL").click();
    h.run();
    assert!(fake.take().is_empty(), "cancelling deletes nothing");
    actions(&mut h, "You", "Thursday works.");
    h.get_by_label("Delete").click();
    h.run();
    h.get_by_label("DELETE").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::Delete { message, .. }] if message.0 == "2"), "{sent:?}");
}

/// Every quick reaction has a glyph in the fonts the app ships (egui's own, until an emoji font is approved).
#[test]
fn the_quick_reactions_are_drawn_by_the_shipped_fonts() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
    out.textures_delta.clear();
    for key in crate::message::QUICK_REACTIONS {
        for c in crate::message::shown_key(key).chars() {
            let has = ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), c));
            assert!(has, "{key:?}: no glyph for {c:?}");
        }
    }
}

#[test]
fn a_quote_goes_to_the_original_or_loads_older_messages() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    h.get_by_label("Reply to Lars Holm: Next meeting on Thursday?").click();
    h.run();
    assert!(fake.take().is_empty(), "the original is loaded: the chat scrolls to it");
    let mut reply = message("5", "Mei Tanaka", false, text("Agreed"), SendState::Sent);
    reply.reply_to = Some(Quote { message: None, sender: "Ana".into(), body: text("Something from last year") });
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: vec![reply] }]);
    h.run();
    h.get_by_label("Reply to Ana: Something from last year").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::LoadOlder(chat)] if chat.0 == "!book"), "{sent:?}");
}

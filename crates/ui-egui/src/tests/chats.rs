//! What can be done with a chat: pinning it to the top, muting it, marking it unread; and pinned messages.

use egui_kittest::kittest::Queryable;
use pingcraft_core::app::{Command, Event};
use pingcraft_core::{ChatId, InboxChange, MessageBody, MessageId, Mute, Quote};

use super::messages::book_club;
use super::{Fake, chat, signed_in};

#[test]
fn the_inbox_says_which_chats_are_pinned_muted_or_marked_unread() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    let mut lars = chat("!lars", "Lars Holm", "matrix", 0, None);
    lars.pinned = true;
    let mut book = chat("!book", "Book club", "Telegram", 2, Some(("Lars", "The ending of chapter 8")));
    book.muted = Some(Mute::UntilUnmuted);
    let mut mei = chat("!mei", "Mei Tanaka", "Telegram", 0, None);
    mei.marked_unread = true;
    fake.tell([Event::Inbox(InboxChange::Reset(vec![lars, book, mei]))]);
    h.run();
    h.get_by_label("Lars Holm, on Matrix, pinned");
    h.get_by_label("Book club, on Telegram, muted, 2 unread. Lars: The ending of chapter 8");
    h.get_by_label("Mei Tanaka, on Telegram, marked unread");
    h.get_by_label("UNREAD 2");
}

fn chat_menu(h: &mut egui_kittest::Harness<'static, crate::PingCraftApp>) {
    h.get_by_label("Chat actions").click();
    h.run();
}

#[test]
fn a_chat_can_be_pinned_muted_and_marked_unread_from_its_menu() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    chat_menu(&mut h);
    h.get_by_label("Pin to the top of the inbox").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::PinChat { chat, pin: true }] if chat.0 == "!book"), "{sent:?}");

    chat_menu(&mut h);
    h.get_by_label("Mute for an hour").click();
    h.run();
    let sent = fake.take();
    let now = chrono::Utc::now().timestamp_millis() as u64;
    assert!(matches!(&sent[..], [Command::Mute { mute: Some(Mute::Until(at)), .. }] if at.0 > now + 3_500_000 && at.0 < now + 3_700_000), "{sent:?}");
    chat_menu(&mut h);
    h.get_by_label("Mute until you turn it back on").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Mute { mute: Some(Mute::UntilUnmuted), .. }]));

    // Once the inbox says it is muted, the menu offers to turn it back on.
    let mut muted = chat("!book", "Book club", "Telegram", 0, None);
    muted.muted = Some(Mute::UntilUnmuted);
    fake.tell([Event::Inbox(InboxChange::Upsert(muted))]);
    h.run();
    chat_menu(&mut h);
    h.get_by_label("Turn notifications back on").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Mute { mute: None, .. }]));

    chat_menu(&mut h);
    h.get_by_label("Mark as unread").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::CloseChat, Command::MarkUnread(chat)] if chat.0 == "!book"), "{sent:?}");
    assert!(h.state().state.open.is_none(), "marking a chat unread leaves it");
}

#[test]
fn pinned_messages_show_in_a_bar_and_can_be_pinned_from_the_menu() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("Pin in this chat").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::PinMessage { message, pin: true, .. }] if message.0 == "1"), "{sent:?}");

    let quote = |id: Option<&str>, sender: &str, text: &str| Quote {
        message: id.map(|i| MessageId(i.into())),
        sender: sender.into(),
        body: MessageBody::Text(text.into()),
    };
    fake.tell([Event::Pinned {
        chat: ChatId("!book".into()),
        pinned: vec![quote(Some("1"), "Lars Holm", "Next meeting on Thursday?"), quote(None, "Ana", "The reading list")],
    }]);
    h.run();
    // The newest pin shows first.
    h.get_by_label("Pinned 1 of 2: Ana: The reading list");
    h.get_by_label("Next pinned message").click();
    h.run();
    h.get_by_label("Pinned 2 of 2: Lars Holm: Next meeting on Thursday?");
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("Unpin").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::PinMessage { message, pin: false, .. }] if message.0 == "1"), "{sent:?}");
}

#[test]
fn a_draft_is_kept_when_leaving_a_chat_and_shown_in_the_inbox() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    super::type_into(&mut h, "Message Book club", "half a thought");
    fake.take();
    h.get_by_label_contains("Lars Holm, on Matrix").click();
    h.run();
    let sent = fake.take();
    assert!(sent.iter().any(|c| matches!(c, Command::SaveDraft { chat, text } if chat.0 == "!book" && text == "half a thought")), "{sent:?}");
    // The inbox hears of it from the store.
    let mut book = chat("!book", "Book club", "Telegram", 0, Some(("Lars", "The ending of chapter 8")));
    book.draft = Some("half a thought".into());
    fake.tell([Event::Inbox(InboxChange::Upsert(book))]);
    h.run();
    h.get_by_label("Book club, on Telegram. Draft: half a thought");
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    assert_eq!(h.state().state.open.as_ref().map(|o| o.draft.as_str()), Some("half a thought"), "the draft comes back");
}

#[test]
fn typing_is_shown_and_told() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    fake.tell([Event::Typing { chat: ChatId("!book".into()), names: vec!["Lars Holm".into()] }]);
    h.run();
    h.get_by_label("Lars Holm is typing…");
    fake.tell([Event::Typing { chat: ChatId("!book".into()), names: vec!["Lars Holm".into(), "Mei Tanaka".into()] }]);
    h.run();
    h.get_by_label("Lars Holm and Mei Tanaka are typing…");
    fake.tell([Event::Typing { chat: ChatId("!book".into()), names: vec![] }]);
    h.run();
    assert!(h.query_by_label_contains("typing…").is_none());

    super::type_into(&mut h, "Message Book club", "on my way");
    let sent = fake.take_all();
    assert!(sent.iter().any(|c| matches!(c, Command::Typing { typing: true, .. })), "{sent:?}");
    h.get_by_label("SEND").click();
    h.run();
    let sent = fake.take_all();
    assert!(sent.iter().any(|c| matches!(c, Command::Typing { typing: false, .. })), "{sent:?}");
}

#[test]
fn jump_to_latest_shows_when_scrolled_up() {
    use pingcraft_core::SendState;
    let fake = Fake::default();
    let mut h = book_club(&fake);
    let many = (0..40).map(|i| super::message(&i.to_string(), "Lars Holm", false, MessageBody::Text(format!("Line {i}")), SendState::Sent)).collect();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: many }]);
    h.run();
    assert!(h.query_by_label("Jump to latest").is_none(), "at the newest message already");
    h.get_by_label("LOAD OLDER MESSAGES").scroll_to_me();
    h.run();
    h.get_by_label("Jump to latest").click();
    h.run();
    assert!(h.query_by_label("Jump to latest").is_none(), "back at the newest message");
}

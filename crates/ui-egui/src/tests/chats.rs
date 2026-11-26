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
    h.get_by_label("Unread 2");
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
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(muted)))]);
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
    assert!(
        sent.iter().any(|c| matches!(c, Command::SaveDraft { chat, text, draft_for: None } if chat.0 == "!book" && text == "half a thought")),
        "{sent:?}"
    );
    // The inbox hears of it from the store.
    let mut book = chat("!book", "Book club", "Telegram", 0, Some(("Lars", "The ending of chapter 8")));
    book.draft = Some("half a thought".into());
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book)))]);
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
    h.get_by_label("Send").click();
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
    h.get_by_label("Load older messages").scroll_to_me();
    h.run();
    h.get_by_label("Jump to latest").click();
    h.run();
    assert!(h.query_by_label("Jump to latest").is_none(), "back at the newest message");
}

#[test]
fn sending_a_kept_draft_forgets_it() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    let mut book = chat("!book", "Book club", "Telegram", 0, None);
    book.draft = Some("see you there".into());
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book)))]);
    h.run();
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.take();
    h.get_by_label("Send").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::SendText { text, .. }, Command::SaveDraft { text: kept, .. }] if text == "see you there" && kept.is_empty()),
        "{sent:?}"
    );
}

/// Archiving a chat from its menu leaves it; archived chats are out of the inbox and its counts, under ARCHIVED,
/// and can be moved back from there; a newer message brings one back by itself.
#[test]
fn a_chat_can_be_archived_and_found_under_archived() {
    use pingcraft_core::Timestamp;
    let fake = Fake::default();
    let mut h = book_club(&fake);
    chat_menu(&mut h);
    h.get_by_label("Archive").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::CloseChat, Command::Archive { chat, archive: true }] if chat.0 == "!book"), "{sent:?}");
    assert!(h.state().state.open.is_none(), "archiving a chat leaves it");

    let mut book = chat("!book", "Book club", "Telegram", 2, Some(("Lars", "The ending of chapter 8")));
    book.archived = Some(Timestamp(1_795_000_000_000));
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book.clone())))]);
    h.run();
    assert!(h.query_by_label_contains("Book club, on Telegram").is_none(), "out of the inbox");
    h.get_by_label("All 1");
    h.get_by_label("Unread 0");
    h.get_by_label("Archived 1").click();
    h.run();
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.take();
    chat_menu(&mut h);
    h.get_by_label("Move back to the inbox").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::Archive { chat, archive: false }] if chat.0 == "!book"), "{sent:?}");

    // A message after it was archived brings it back by itself.
    h.get_by_label("All 1").click();
    h.run();
    let mut newer = chat("!book", "Book club", "Telegram", 3, Some(("Lars", "Chapter 9 now")));
    newer.archived = book.archived;
    newer.last_message.as_mut().unwrap().at = Timestamp(1_795_000_000_001);
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(newer)))]);
    h.run();
    h.get_by_label_contains("Book club, on Telegram");
    h.get_by_label("Archived 0");
}

/// The draft that is kept, as the backend was asked to keep it: its text and what it is for.
fn kept_draft(fake: &Fake) -> Option<(String, Option<pingcraft_core::DraftFor>)> {
    fake.take().into_iter().rev().find_map(|c| match c {
        Command::SaveDraft { text, draft_for, .. } => Some((text, draft_for)),
        _ => None,
    })
}

/// A reply in progress is kept with the draft (by the message's event ID), and comes back with it: the reply bar
/// over the composer once the message is loaded.
#[test]
fn a_reply_in_progress_is_kept_with_the_draft() {
    use pingcraft_core::DraftFor;
    let fake = Fake::default();
    let mut h = book_club(&fake);
    h.get_by_label("Message actions: Lars Holm, Next meeting on Thursday?").click();
    h.run();
    h.get_by_label("Reply").click();
    h.run();
    super::type_into(&mut h, "Message Book club", "yes, Thursday");
    fake.take();
    h.get_by_label_contains("Lars Holm, on Matrix").click();
    h.run();
    assert_eq!(kept_draft(&fake), Some(("yes, Thursday".into(), Some(DraftFor::Reply("$lars".into())))));

    let mut book = chat("!book", "Book club", "Telegram", 0, Some(("Lars", "The ending of chapter 8")));
    book.draft = Some("yes, Thursday".into());
    book.draft_for = Some(DraftFor::Reply("$lars".into()));
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book)))]);
    h.run();
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    let messages = super::messages::book_club_messages();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages }]);
    h.run();
    h.get_by_label("Replying to Lars Holm");
    fake.take();
    h.get_by_label("Send").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::Reply { to, text, .. }, Command::SaveDraft { text: kept, .. }] if to.0 == "1" && text == "yes, Thursday" && kept.is_empty()),
        "{sent:?}"
    );
}

/// An edit in progress is kept as the draft and comes back as an edit. An edit left unchanged is no work in
/// progress: the draft set aside for it is kept instead.
#[test]
fn an_edit_in_progress_is_kept_with_the_draft() {
    use pingcraft_core::DraftFor;
    let fake = Fake::default();
    let mut h = book_club(&fake);
    super::type_into(&mut h, "Message Book club", "set aside");
    h.get_by_label("Message actions: You, Thursday works.").click();
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    fake.take();
    h.get_by_label_contains("Lars Holm, on Matrix").click();
    h.run();
    assert_eq!(kept_draft(&fake), Some(("set aside".into(), None)), "an unchanged edit keeps the set-aside draft");

    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: super::messages::book_club_messages() }]);
    h.run();
    h.get_by_label("Message actions: You, Thursday works.").click();
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    // Typed at the end of the text (where the cursor lands when focused is egui's business).
    h.state_mut().state.open.as_mut().unwrap().draft.push_str(" Friday too.");
    h.run();
    fake.take();
    h.get_by_label_contains("Lars Holm, on Matrix").click();
    h.run();
    assert_eq!(kept_draft(&fake), Some(("Thursday works. Friday too.".into(), Some(DraftFor::Edit("$mine".into())))));

    let mut book = chat("!book", "Book club", "Telegram", 0, None);
    book.draft = Some("Thursday works. Friday too.".into());
    book.draft_for = Some(DraftFor::Edit("$mine".into()));
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book)))]);
    h.run();
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: super::messages::book_club_messages() }]);
    h.run();
    h.get_by_label("Editing your message");
    fake.take();
    h.get_by_label("Save").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::Edit { message, text, .. }, ..] if message.0 == "2" && text == "Thursday works. Friday too."), "{sent:?}");
    assert_eq!(h.state().state.open.as_ref().map(|o| o.draft.as_str()), Some(""), "the composer is empty once the edit is saved");
}

/// A restored reply to a message that is not among the loaded ones is dropped, and its text stays as a new draft.
#[test]
fn a_restored_reply_to_a_message_not_loaded_keeps_its_text() {
    use pingcraft_core::DraftFor;
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    let mut book = chat("!book", "Book club", "Telegram", 0, None);
    book.draft = Some("about that".into());
    book.draft_for = Some(DraftFor::Reply("$gone".into()));
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(book)))]);
    h.run();
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: vec![] }]);
    h.run();
    fake.tell([Event::Messages { chat: ChatId("!book".into()), messages: super::messages::book_club_messages() }]);
    h.run();
    assert!(h.query_by_label("Replying to Lars Holm").is_none());
    assert_eq!(h.state().state.open.as_ref().map(|o| o.draft.as_str()), Some("about that"));
}

/// The open chat's draft is kept when PingCraft closes.
#[test]
fn the_open_chats_draft_is_kept_when_pingcraft_closes() {
    let fake = Fake::default();
    let mut h = book_club(&fake);
    super::type_into(&mut h, "Message Book club", "unfinished");
    fake.take_all();
    h.state_mut().closing();
    let sent = fake.take_all();
    // Only the draft: telling the chat the user stopped typing is a request the closing window would wait on.
    assert!(matches!(&sent[..], [Command::SaveDraft { text, draft_for: None, .. }] if text == "unfinished"), "{sent:?}");
}

/// A chat's notification sound and accent colour are set from its menu; unset, they follow the network's, which the
/// ACCOUNTS tab sets. Notifications sound as the chat's and its network's settings say.
#[test]
fn a_chats_sound_and_colour_follow_its_network_unless_set() {
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::kittest::NodeT;
    use pingcraft_core::{Accent, Alerts, Network};
    let fake = Fake::default();
    let mut h = book_club(&fake);
    chat_menu(&mut h);
    assert_eq!(h.get_by_role_and_label(Role::CheckBox, "Notification sound").accesskit_node().toggled(), Some(Toggled::True), "on by default");
    h.get_by_role_and_label(Role::CheckBox, "Notification sound").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::ChatAlerts { chat, alerts: Alerts { sound: Some(false), accent: None } }] if chat.0 == "!book"),
        "{sent:?}"
    );
    chat_menu(&mut h);
    h.get_by_role_and_label(Role::RadioButton, "Accent colour: Signal's").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::ChatAlerts { alerts: Alerts { accent: Some(Accent::Signal), .. }, .. }]), "{sent:?}");

    // The network turns sound off; a chat without its own setting follows it.
    let mut networks = std::collections::BTreeMap::new();
    networks.insert(Network::from_id("telegram"), Alerts { sound: Some(false), accent: Some(Accent::Discord) });
    fake.tell([Event::NetworkAlerts(networks)]);
    h.run();
    chat_menu(&mut h);
    assert_eq!(h.get_by_role_and_label(Role::CheckBox, "Notification sound").accesskit_node().toggled(), Some(Toggled::False));
    assert_eq!(h.get_by_role_and_label(Role::RadioButton, "Accent colour: the network's").accesskit_node().toggled(), Some(Toggled::True));
    h.key_press(egui::Key::Escape);
    h.run();

    // A new message in another chat on that network: a notification without sound.
    let notifier = super::FakeNotifier::default();
    h.state_mut().set_notifier(Box::new(notifier.clone()));
    let mut mei = chat("!mei", "Mei Tanaka", "Telegram", 0, Some(("Mei", "hi")));
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(mei.clone())))]);
    h.run();
    mei.unread = 1;
    mei.last_message.as_mut().unwrap().at.0 += 1;
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(mei.clone())))]);
    h.run();
    let shown = notifier.shown.borrow().clone();
    assert_eq!(shown.iter().map(|n| n.sound).collect::<Vec<_>>(), [false]);
    // Its own setting wins.
    mei.alerts.sound = Some(true);
    mei.unread = 2;
    mei.last_message.as_mut().unwrap().at.0 += 1;
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(mei)))]);
    h.run();
    assert_eq!(notifier.shown.borrow().iter().map(|n| n.sound).collect::<Vec<_>>(), [false, true]);
}

/// The ACCOUNTS tab sets each network's notification sound and accent colour.
#[test]
fn a_networks_sound_and_colour_are_set_on_the_accounts_tab() {
    use egui::accesskit::Role;
    use pingcraft_core::{Accent, Alerts, Network};
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label("Accounts").click();
    h.run();
    fake.take();
    // Below the cards and the accounts: scrolled to, as a screen reader would.
    h.get_by_role_and_label(Role::CheckBox, "Notification sound for Telegram").scroll_to_me();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "Notification sound for Telegram").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::NetworkAlerts { network, alerts: Alerts { sound: Some(false), accent: None } }] if *network == Network::from_id("telegram")),
        "{sent:?}"
    );
    // A second change before the backend answers builds on the first.
    h.get_by_role_and_label(Role::RadioButton, "Accent colour for Telegram: Discord's").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::NetworkAlerts { alerts: Alerts { sound: Some(false), accent: Some(Accent::Discord) }, .. }]), "{sent:?}");
    h.get_by_role_and_label(Role::RadioButton, "Accent colour for Signal: WhatsApp's").scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::RadioButton, "Accent colour for Signal: WhatsApp's").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::NetworkAlerts { alerts: Alerts { accent: Some(Accent::WhatsApp), .. }, .. }]), "{sent:?}");
}

/// Enter in the composer sends what was typed.
#[test]
fn enter_in_the_composer_sends() {
    let fake = Fake::default();
    let mut h = super::messages::book_club(&fake);
    fake.take();
    super::type_into(&mut h, "Message Book club", "on my way");
    h.key_press(egui::Key::Enter);
    h.run();
    let sent = fake.take();
    assert!(sent.iter().any(|c| matches!(c, Command::SendText { text, .. } if text == "on my way")), "{sent:?}");
}

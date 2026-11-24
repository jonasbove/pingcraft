//! The screens through their AccessKit tree, against a fake backend: what a screen reader would find, and the
//! commands a user's clicks and typing send.

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pingcraft_core::app::{Backend, Command, Event, SignInProblem};
use pingcraft_core::encryption::{CancelReason, RecoveryKey, RecoveryStatus, SasSymbol, Verification};
use pingcraft_core::{Chat, ChatId, ConnectionState, InboxChange, Message, MessageBody, MessageId, MessagePreview, Network, SendState, Timestamp};

use crate::PingCraftApp;

/// A backend that records commands and hands out queued events.
#[derive(Clone, Default)]
struct Fake {
    sent: Rc<RefCell<Vec<Command>>>,
    queue: Rc<RefCell<Vec<Event>>>,
}

impl Backend for Fake {
    fn send(&mut self, command: Command) {
        self.sent.borrow_mut().push(command);
    }

    fn poll(&mut self) -> Vec<Event> {
        std::mem::take(&mut *self.queue.borrow_mut())
    }
}

impl Fake {
    fn tell(&self, events: impl IntoIterator<Item = Event>) {
        self.queue.borrow_mut().extend(events);
    }

    fn take(&self) -> Vec<Command> {
        std::mem::take(&mut *self.sent.borrow_mut())
    }
}

fn harness(fake: &Fake) -> Harness<'static, PingCraftApp> {
    let app = PingCraftApp::new(Box::new(fake.clone()));
    Harness::builder().with_size(egui::vec2(1200.0, 760.0)).build_ui_state(|ui, app: &mut PingCraftApp| app.ui(ui), app)
}

/// Focuses the text field with this accessible name and types into it, as a user would.
fn type_into(h: &mut Harness<'static, PingCraftApp>, label: &str, text: &str) {
    use egui::accesskit::Role;
    let role = if h.query_by_role_and_label(Role::PasswordInput, label).is_some() { Role::PasswordInput } else { Role::TextInput };
    h.get_by_role_and_label(role, label).focus();
    h.run();
    h.get_by_role_and_label(role, label).type_text(text);
    h.run();
}

fn chat(id: &str, name: &str, network: &str, unread: u32, preview: Option<(&str, &str)>) -> Chat {
    Chat {
        id: ChatId(id.into()),
        account: None,
        network: Network::from_id(network),
        name: name.into(),
        last_message: preview.map(|(sender, text)| MessagePreview {
            sender: sender.into(),
            own: false,
            body: MessageBody::Text(text.into()),
            at: Timestamp(1_795_000_000_000),
        }),
        unread,
        encrypted: true,
    }
}

fn message(id: &str, sender: &str, own: bool, body: MessageBody, state: SendState) -> Message {
    Message { id: MessageId(id.into()), sender: sender.into(), own, at: Timestamp(1_795_000_000_000), body, state }
}

fn signed_in(fake: &Fake) -> Harness<'static, PingCraftApp> {
    let mut h = harness(fake);
    fake.tell([
        Event::SignedIn { user: "@Ana:example.org".into() },
        Event::Connection(ConnectionState::Online),
        Event::Recovery(RecoveryStatus::On),
        Event::Inbox(InboxChange::Reset(vec![
            chat("!book", "Book club", "Telegram", 2, Some(("Lars", "The ending of chapter 8"))),
            chat("!lars", "Lars Holm", "matrix", 0, None),
        ])),
    ]);
    h.run();
    h
}

#[test]
fn signing_in_sends_what_was_typed_and_says_why_it_failed() {
    let fake = Fake::default();
    let mut h = harness(&fake);
    fake.tell([Event::SignedOut]);
    h.run();
    h.get_by_label("Not signed in");
    assert!(h.get_by_label("SIGN IN").accesskit_node().is_disabled(), "nothing to send yet");

    type_into(&mut h, "SERVER ADDRESS", "example.org");
    type_into(&mut h, "USERNAME", "ana");
    type_into(&mut h, "PASSWORD", "hunter2");
    h.run();
    h.get_by_label("SIGN IN").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::SignIn { server, username, password }] if server == "example.org" && username == "ana" && **password == "hunter2"),
        "{sent:?}"
    );
    assert!(h.state().state.sign_in.password.is_empty(), "the password leaves the form once sent");

    fake.tell([Event::SigningIn, Event::SignInFailed(SignInProblem::WrongPassword)]);
    h.run();
    h.get_by_label("The server did not accept that username and password.");
}

#[test]
fn a_second_pingcraft_says_one_is_already_open_and_offers_nothing_to_type() {
    let fake = Fake::default();
    let mut h = harness(&fake);
    fake.tell([Event::AlreadyRunning]);
    h.run();
    h.get_by_label("PingCraft is already open");
    h.get_by_label("Another PingCraft window is using your messages on this computer.");
    h.get_by_label("Already open");
    use egui::accesskit::Role;
    assert!(h.query_by_role(Role::TextInput).is_none() && h.query_by_role(Role::PasswordInput).is_none(), "no sign-in or passphrase field");
    assert!(fake.take().is_empty());
}

#[test]
fn choosing_a_passphrase_needs_it_twice_and_the_same() {
    let fake = Fake::default();
    let mut h = harness(&fake);
    fake.tell([Event::Locked { first: true }]);
    h.run();
    assert!(h.query_all_by_label("Locked").count() >= 1, "the header says PingCraft is locked");
    type_into(&mut h, "PASSPHRASE", "correct horse");
    type_into(&mut h, "THE SAME PASSPHRASE AGAIN", "correct hors");
    h.run();
    h.get_by_label("The two passphrases are not the same.");
    assert!(h.get_by_label("CONTINUE").accesskit_node().is_disabled());
    type_into(&mut h, "THE SAME PASSPHRASE AGAIN", "e");
    h.run();
    h.get_by_label("CONTINUE").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Unlock { passphrase }] if **passphrase == "correct horse"));
}

#[test]
fn a_wrong_unlock_passphrase_is_said_and_signing_out_is_offered() {
    let fake = Fake::default();
    let mut h = harness(&fake);
    fake.tell([Event::Locked { first: false }, Event::WrongPassphrase]);
    h.run();
    h.get_by_label("That passphrase does not unlock PingCraft.");
    // Signing out here clears this computer, so it is asked first, and can be taken back.
    h.get_by_label("SIGN OUT").click();
    h.run();
    assert!(fake.take().is_empty(), "nothing is cleared on the first click");
    h.get_by_label("Sign out on this computer?");
    h.get_by_label("CANCEL").click();
    h.run();
    h.get_by_label("Unlock PingCraft");
    h.get_by_label("SIGN OUT").click();
    h.run();
    h.get_by_label("CLEAR AND SIGN OUT").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::SignOut]));
}

#[test]
fn the_inbox_reads_each_chat_and_opens_one() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label("Connected");
    // The footer shows the Matrix ID as typed, not in capitals.
    h.get_by_label_contains("@Ana:example.org");
    h.get_by_label("ALL 2");
    h.get_by_label("UNREAD 1");
    let row = h.get_by_label("Book club, on Telegram, 2 unread. Lars: The ending of chapter 8");
    row.click();
    h.get_by_label("Lars Holm, on Matrix");
    h.run();
    assert!(matches!(&fake.take()[..], [Command::OpenChat(id)] if id.0 == "!book"));

    h.get_by_label("UNREAD 1").click();
    h.run();
    assert!(h.query_by_label("Lars Holm, on Matrix").is_none(), "read chats leave the unread filter");
}

#[test]
fn the_open_chat_shows_every_message_state_and_sends() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    fake.take();
    fake.tell([Event::Messages {
        chat: ChatId("!book".into()),
        messages: vec![
            message("1", "Lars Holm", false, MessageBody::Text("Next meeting on Thursday?".into()), SendState::Sent),
            message("2", "Lars Holm", false, MessageBody::WaitingForKey, SendState::Sent),
            message("3", "@Ana:example.org", true, MessageBody::Text("Thursday works.".into()), SendState::Sent),
            message("4", "@Ana:example.org", true, MessageBody::Text("Bringing bread".into()), SendState::Failed("timeout".into())),
            message("5", "Lars Holm", false, MessageBody::Unsupported("image".into()), SendState::Sent),
        ],
    }]);
    h.run();
    h.get_by_label("Encrypted · via your bridge");
    h.get_by_label("Next meeting on Thursday?");
    h.get_by_label("Waiting for the key to this message");
    h.get_by_label("SENT");
    h.get_by_label("NOT SENT");
    h.get_by_label("An image");
    h.get_by_label("RETRY").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Retry { chat, message }] if chat.0 == "!book" && message.0 == "4"));

    // It is at the top of the timeline, which sticks to the newest message: scroll up to it first.
    h.get_by_label("LOAD OLDER MESSAGES").scroll_to_me();
    h.run();
    h.get_by_label("LOAD OLDER MESSAGES").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::LoadOlder(chat)] if chat.0 == "!book"), "{sent:?}");
    fake.tell([Event::ReachedStart(ChatId("!book".into()))]);
    h.run();
    assert!(h.query_by_label("LOAD OLDER MESSAGES").is_none(), "nothing older to load");

    type_into(&mut h, "Message Book club", "See you then");
    h.run();
    h.get_by_label("SEND").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::SendText { chat, text }] if chat.0 == "!book" && text == "See you then"));
}

#[test]
fn the_recovery_key_must_be_saved_before_continuing() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    let key = "EsTc 8Jq2 Lm4P vR7x Wn3K hY6d Qa9B tF2s Zp5G kU8e Nr4V jD7w";
    fake.tell([Event::NewRecoveryKey(RecoveryKey::new(key))]);
    h.run();
    h.get_by_label("Save your recovery key");
    h.get_by_label("EsTc 8Jq2 Lm4P vR7x Wn3K hY6d");
    h.get_by_label("Qa9B tF2s Zp5G kU8e Nr4V jD7w");
    assert!(h.get_by_label("CONTINUE").accesskit_node().is_disabled());
    h.get_by_label("I've saved my recovery key somewhere safe, away from this computer.").click();
    h.run();
    h.get_by_label("CONTINUE").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::RecoveryKeySaved]));
    assert!(h.query_by_label("Save your recovery key").is_none());
}

#[test]
fn a_device_without_the_key_can_type_it_or_be_verified() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    fake.tell([Event::Recovery(RecoveryStatus::NeedsKey)]);
    h.run();
    h.get_by_label("This device can't read your earlier encrypted messages yet.");
    h.get_by_label("ENTER RECOVERY KEY").click();
    h.run();
    type_into(&mut h, "RECOVERY KEY", "EsTc 8Jq2");
    h.run();
    h.get_by_label("USE THIS KEY").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Recover(key)] if key.expose() == "EsTc 8Jq2"));
    fake.tell([Event::WrongRecoveryKey]);
    h.run();
    h.get_by_label("That key doesn't open your key backup. Check it and try again.");
    h.get_by_label("CANCEL").click();
    h.run();
    h.get_by_label("VERIFY THIS DEVICE").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::VerifyThisDevice]));
}

#[test]
fn verifying_compares_seven_named_symbols() {
    let fake = Fake::default();
    // egui's bundled emoji face lacks some of the spec's symbols; the app needs a fuller one (see PLAN.md).
    let app = PingCraftApp::new(Box::new(fake.clone()));
    let mut h =
        Harness::builder().with_size(egui::vec2(1200.0, 760.0)).allow_missing_glyphs().build_ui_state(|ui, app: &mut PingCraftApp| app.ui(ui), app);
    fake.tell([Event::SignedIn { user: "@ana:example.org".into() }, Event::Inbox(InboxChange::Reset(vec![]))]);
    h.run();
    let names = ["Dog", "Cat", "Lion", "Horse", "Unicorn", "Pig", "Elephant"];
    let symbols = ["🐶", "🐱", "🦁", "🐎", "🦄", "🐷", "🐘"];
    let compare =
        names.iter().zip(symbols).enumerate().map(|(i, (n, s))| SasSymbol { index: i as u8, symbol: s.into(), name: (*n).into() }).collect();
    fake.tell([Event::Verification(Some(Verification::Incoming { device: "Laptop".into() }))]);
    h.run();
    h.get_by_label("Your device Laptop asks to be verified. Start only if you asked for it yourself, on that device.");
    h.get_by_label("START").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::AcceptVerification]));

    fake.tell([Event::Verification(Some(Verification::Compare(compare)))]);
    h.run();
    for name in names {
        h.get_by_label(name);
    }
    h.get_by_label("THEY DON'T MATCH");
    // One more frame: the sheet grew with the symbols and re-centres a frame later, so the button moves once
    // more after run() settles. (Found by testing: without it the click lands where the button was.)
    h.step();
    h.get_by_label("THEY MATCH").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::SymbolsMatch]), "{sent:?}");

    fake.tell([Event::Verification(Some(Verification::Cancelled(CancelReason::Mismatch)))]);
    h.run();
    h.get_by_label("Stopped: the symbols didn't match. Nothing was shared.");
    h.get_by_label("CLOSE").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::CancelVerification]));
}

#[test]
fn copying_the_recovery_key_says_so() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    fake.tell([Event::NewRecoveryKey(RecoveryKey::new("EsTc 8Jq2 Lm4P vR7x Wn3K hY6d Qa9B tF2s Zp5G kU8e Nr4V jD7w"))]);
    h.run();
    assert!(h.query_by_label("COPIED").is_none());
    h.get_by_label("COPY").click();
    h.run();
    h.get_by_label("COPIED");
}

#[test]
fn a_long_chat_name_truncates_and_keeps_its_network_tag_on_the_same_line() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    let long = "m1.pingcraft.test Admin Room with a much longer name than fits in the list";
    fake.tell([Event::Inbox(InboxChange::Upsert(chat("!admin", long, "matrix", 0, None)))]);
    h.run();
    let row = h.get_by_label(&format!("{long}, on Matrix")).accesskit_node().raw_bounds().unwrap();
    let tag = h.get_all_by_label("MATRIX").map(|n| n.accesskit_node().raw_bounds().unwrap()).find(|r| r.y0 >= row.y0 && r.y1 <= row.y1).unwrap();
    assert!(tag.x1 <= row.x1, "the tag stays inside the row");
    let middle = (row.y0 + row.y1) / 2.0;
    assert!(tag.y0 < middle && tag.y1 > middle, "a row without a preview centres its one line: tag {tag:?}, row {row:?}");
}

#[test]
fn a_chat_row_keeps_its_preview_inside_the_row() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    fake.tell([Event::Inbox(InboxChange::Upsert(chat("!admin", "m1.pingcraft.test Admin Room", "matrix", 0, Some(("You", "This works?")))))]);
    h.run();
    let row = h.get_by_label("m1.pingcraft.test Admin Room, on Matrix. You: This works?").accesskit_node().raw_bounds().unwrap();
    for label in ["m1.pingcraft.test Admin Room", "You: This works?"] {
        let r = h
            .get_all_by_label(label)
            .map(|n| n.accesskit_node().raw_bounds().unwrap())
            .find(|r| r.x0 >= row.x0 && r.x1 <= row.x1 && r.y0 >= row.y0 - 40.0 && r.y0 <= row.y1)
            .unwrap();
        assert!(r.y0 >= row.y0 && r.y1 <= row.y1, "{label} spills out of its row: {r:?} vs {row:?}");
    }
}

#[test]
fn the_chat_header_centres_the_name_and_network_on_the_avatar() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label_contains("Book club, on Telegram").click();
    h.run();
    // The encryption badge is centred in the header by its layout, so it marks the avatar's centre line.
    let badge = h.get_by_label("Encrypted · via your bridge").accesskit_node().raw_bounds().unwrap();
    let name =
        h.get_all_by_label("Book club").map(|n| n.accesskit_node().raw_bounds().unwrap()).find(|r| r.y0 < badge.y1 + 20.0 && r.x0 > 400.0).unwrap();
    let tag = h.get_all_by_label("TELEGRAM").map(|n| n.accesskit_node().raw_bounds().unwrap()).find(|r| r.x0 > 400.0).unwrap();
    let block = (name.y0 + tag.y1) / 2.0;
    let line = (badge.y0 + badge.y1) / 2.0;
    assert!((block - line).abs() <= 2.0, "name and tag centred at {block}, the header's centre line is {line}");
}

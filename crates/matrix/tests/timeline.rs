//! An open chat against a real homeserver: reading, sending, retrying, older messages and messages whose key has
//! not arrived. See `common/mod.rs` for how to point it at one.

mod common;

use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use pingcraft_core::{ChatId, Inbox, Message, MessageBody, SendState};
use pingcraft_matrix::{ChatTimeline, Connection};
use serde_json::json;

use common::{Device, chat_until, create_room, inbox_until, join_and_say, sign_in_as, state, unique, within};

/// Starts syncing as `device` and opens `room` once the inbox has it.
async fn open(device: &Device, room: &OwnedRoomId) -> (Connection, ChatTimeline) {
    let connection = within("the sync to start", Connection::start(device.session.client.clone())).await.unwrap();
    let mut feed = connection.inbox().await.unwrap();
    let mut inbox = Inbox::default();
    let id = ChatId(room.to_string());
    inbox_until("the chat in the inbox", &mut feed, &mut inbox, |inbox| inbox.get(&id).map(|_| ())).await;
    let chat = within("the chat to open", connection.open_chat(&id)).await.unwrap();
    (connection, chat)
}

fn text(m: &Message) -> Option<&str> {
    match &m.body {
        MessageBody::Text(t) => Some(t),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sending_and_reading_text_in_an_encrypted_chat() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let ben = sign_in_as(&server.homeserver, &server.other).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();
    let room = create_room(&ana, &unique("Chat"), true, Some(ben_id), vec![]).await;
    let (connection, mut chat) = open(&ana, &room).await;
    let mut last = Vec::new();

    chat.send_text("hi ben").await.unwrap();
    // The local echo shows at once; the same message (same ID) turns into a sent one.
    let echo = chat_until("the local echo", &mut chat, &mut last, |ms| ms.iter().find(|m| text(m) == Some("hi ben")).cloned()).await;
    assert!(echo.own);
    let sent = chat_until("the message to be sent", &mut chat, &mut last, |ms| {
        ms.iter().find(|m| text(m) == Some("hi ben") && m.state == SendState::Sent).cloned()
    })
    .await;
    assert_eq!(sent.id, echo.id, "a message keeps its ID from local echo to sent");

    join_and_say(&ben, &room, "hello ana").await;
    let reply = chat_until("ben's reply", &mut chat, &mut last, |ms| ms.iter().find(|m| text(m) == Some("hello ana")).cloned()).await;
    assert!(!reply.own);
    assert_eq!(reply.state, SendState::Sent);
    let order: Vec<_> = last.iter().filter_map(text).collect();
    assert_eq!(order, ["hi ben", "hello ana"], "oldest first");
    chat.mark_read().await.unwrap();

    drop(chat);
    connection.stop().await;
    ana.sign_out().await;
    ben.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn older_messages_load_back_to_the_start() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let room = create_room(&ana, &unique("History"), false, None, vec![]).await;
    let sdk_room = ana.session.client.get_room(&room).unwrap();
    for i in 0..40 {
        sdk_room.send(RoomMessageEventContent::text_plain(format!("message {i}"))).await.unwrap();
    }
    let (connection, mut chat) = open(&ana, &room).await;
    let mut last = Vec::new();
    chat_until("the newest message", &mut chat, &mut last, |ms| ms.iter().any(|m| text(m) == Some("message 39")).then_some(())).await;
    assert!(!last.iter().any(|m| text(m) == Some("message 0")), "the first message is not loaded yet");

    let mut at_start = false;
    for _ in 0..20 {
        at_start = within("an older page", chat.load_older(10)).await.unwrap();
        if at_start {
            break;
        }
    }
    assert!(at_start, "paginating reaches the start of the chat");
    chat_until("the first message", &mut chat, &mut last, |ms| ms.iter().any(|m| text(m) == Some("message 0")).then_some(())).await;
    let numbers: Vec<_> = last.iter().filter_map(text).filter(|t| t.starts_with("message ")).collect();
    assert_eq!(numbers.len(), 40, "every message once: {numbers:?}");
    assert_eq!(numbers.first(), Some(&"message 0"));

    drop(chat);
    connection.stop().await;
    ana.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_the_server_refused_can_be_sent_again() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let ben = sign_in_as(&server.homeserver, &server.other).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();
    // Only ana, who made the room, may speak at first. (Room creators are not listed in `users`.)
    let levels = |ben_level: i64| json!({"users": {ben_id.as_str(): ben_level}, "events_default": 50, "state_default": 50, "invite": 0});
    let room = create_room(&ana, &unique("Moderated"), false, Some(ben_id.clone()), vec![state("m.room.power_levels", "", levels(0))]).await;
    ben.session.client.join_room_by_id(&room).await.unwrap();

    let (connection, mut chat) = open(&ben, &room).await;
    let mut last = Vec::new();
    chat.send_text("may I?").await.unwrap();
    let failed = chat_until("the send to fail", &mut chat, &mut last, |ms| {
        ms.iter().find(|m| text(m) == Some("may I?") && matches!(m.state, SendState::Failed(_))).cloned()
    })
    .await;

    ana.session.client.get_room(&room).unwrap().send_state_event_raw("m.room.power_levels", "", levels(50)).await.unwrap();
    // Ben's client has to see the new power levels before the server accepts; retrying is what the user does.
    chat.retry(&failed.id).await.unwrap();
    let sent = within("the retry to go through", async {
        loop {
            last.clear();
            let outcome = chat_until("the retry's outcome", &mut chat, &mut last, |ms| {
                ms.iter().find(|m| m.id == failed.id && m.state != SendState::Sending).map(|m| m.state.clone())
            })
            .await;
            if outcome == SendState::Sent {
                return outcome;
            }
            // Still refused: the server may not have applied the new power levels yet. The message stays failed,
            // so the user (here, the test) tries again. A list from before the retry can still say "failed", in
            // which case there is nothing to retry yet.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = chat.retry(&failed.id).await;
        }
    })
    .await;
    assert_eq!(sent, SendState::Sent);

    drop(chat);
    connection.stop().await;
    ana.sign_out().await;
    ben.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_from_before_this_device_waits_for_its_key() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let ben = sign_in_as(&server.homeserver, &server.other).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();
    let room = create_room(&ana, &unique("Before"), true, Some(ben_id), vec![]).await;
    join_and_say(&ben, &room, "said before the new device").await;

    // Ben on a new device, with no key backup: the old message cannot be read there.
    let ben_new = sign_in_as(&server.homeserver, &server.other).await;
    let (connection, mut chat) = open(&ben_new, &room).await;
    let mut last = Vec::new();
    chat_until("the undecryptable message", &mut chat, &mut last, |ms| ms.iter().any(|m| m.body == MessageBody::WaitingForKey).then_some(())).await;

    drop(chat);
    connection.stop().await;
    ana.sign_out().await;
    ben.sign_out().await;
    ben_new.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn replying_reacting_editing_and_deleting() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let ben = sign_in_as(&server.homeserver, &server.other).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();
    let room = create_room(&ana, &unique("Actions"), true, Some(ben_id), vec![]).await;
    // Ana's new device syncs (and so publishes its keys) before Ben writes; otherwise Ben's app has no device of
    // hers to encrypt for, and the message waits for a key that never comes.
    let (_connection, mut chat) = open(&ana, &room).await;
    join_and_say(&ben, &room, "lunch?").await;
    let mut last = Vec::new();
    let theirs = chat_until("ben's message", &mut chat, &mut last, |ms| ms.iter().find(|m| text(m) == Some("lunch?")).cloned()).await;
    assert!(theirs.can.reply && theirs.can.react && !theirs.can.edit && !theirs.can.delete);

    chat.reply(&theirs.id, "yes, at noon").await.unwrap();
    let reply = chat_until("the reply", &mut chat, &mut last, |ms| {
        ms.iter().find(|m| text(m) == Some("yes, at noon") && m.state == SendState::Sent && m.reply_to.is_some()).cloned()
    })
    .await;
    let quote = reply.reply_to.clone().unwrap();
    assert_eq!(quote.message.as_ref(), Some(&theirs.id), "the quote points at the original");
    assert_eq!(quote.body, MessageBody::Text("lunch?".into()));
    assert!(reply.can.edit && reply.can.delete);

    chat.react(&theirs.id, "👍").await.unwrap();
    chat_until("the reaction", &mut chat, &mut last, |ms| ms.iter().find(|m| m.id == theirs.id && m.my_reaction("👍")).map(|_| ())).await;
    chat.react(&theirs.id, "👍").await.unwrap();
    chat_until("the reaction taken back", &mut chat, &mut last, |ms| ms.iter().find(|m| m.id == theirs.id && m.reactions.is_empty()).map(|_| ()))
        .await;

    chat.edit(&reply.id, "yes, at one").await.unwrap();
    chat_until("the edit", &mut chat, &mut last, |ms| ms.iter().find(|m| m.id == reply.id && m.edited && text(m) == Some("yes, at one")).map(|_| ()))
        .await;

    chat.delete(&reply.id).await.unwrap();
    chat_until("the deletion", &mut chat, &mut last, |ms| ms.iter().find(|m| m.id == reply.id && m.body == MessageBody::Deleted).map(|_| ())).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn details_show_edits_and_forwarding_sends_copies() {
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let room = create_room(&ana, &unique("Details"), true, None, vec![]).await;
    let other = create_room(&ana, &unique("Elsewhere"), true, None, vec![]).await;
    let (connection, mut chat) = open(&ana, &room).await;
    let mut last = Vec::new();
    chat.send_text("first words").await.unwrap();
    let sent = chat_until("the message", &mut chat, &mut last, |ms| {
        ms.iter().find(|m| text(m) == Some("first words") && m.state == SendState::Sent).cloned()
    })
    .await;
    chat.edit(&sent.id, "second words").await.unwrap();
    chat_until("the edit", &mut chat, &mut last, |ms| ms.iter().find(|m| m.id == sent.id && m.edited).map(|_| ())).await;
    let details = chat.details(&sent.id).await.unwrap();
    assert!(details.encrypted);
    let versions: Vec<_> = details.versions.iter().map(|(_, b)| b.clone()).collect();
    assert_eq!(versions, [MessageBody::Text("first words".into()), MessageBody::Text("second words".into())]);

    for text in chat.texts(std::slice::from_ref(&sent.id)).await {
        connection.send_text_to(&pingcraft_core::ChatId(other.to_string()), &text).await.unwrap();
    }
    drop(chat);
    let mut elsewhere = within("the other chat to open", connection.open_chat(&pingcraft_core::ChatId(other.to_string()))).await.unwrap();
    let mut last = Vec::new();
    chat_until("the forwarded copy", &mut elsewhere, &mut last, |ms| ms.iter().find(|m| text(m) == Some("second words")).map(|_| ())).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pins_mutes_and_marking_unread_reach_the_server() {
    use pingcraft_core::{ChatId, Mute, Timestamp};
    use pingcraft_matrix::ChatUpdate;
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let room = create_room(&ana, &unique("Settings"), true, None, vec![]).await;
    let (connection, mut chat) = open(&ana, &room).await;
    let id = ChatId(room.to_string());
    let mut last = Vec::new();
    chat.send_text("pin me").await.unwrap();
    let sent =
        chat_until("the message", &mut chat, &mut last, |ms| ms.iter().find(|m| text(m) == Some("pin me") && m.state == SendState::Sent).cloned())
            .await;
    chat.pin(&sent.id, true).await.unwrap();
    within("the pin", async {
        loop {
            if let Some(ChatUpdate::Pinned(p)) = chat.next_update().await
                && p.iter().any(|q| q.message.as_ref() == Some(&sent.id))
            {
                return;
            }
        }
    })
    .await;

    assert!(connection.pin_chat(&id, true).await.unwrap().pinned);
    let until = Timestamp(4_000_000_000_000);
    assert_eq!(connection.mute(&id, Some(Mute::Until(until))).await.unwrap().muted, Some(Mute::Until(until)));
    assert!(connection.mark_unread(&id).await.unwrap().marked_unread);
    let mut feed = connection.inbox().await.unwrap();
    let mut inbox = pingcraft_core::Inbox::default();
    inbox_until("the settings in the inbox", &mut feed, &mut inbox, |inbox| {
        inbox.get(&id).filter(|c| c.pinned && c.muted == Some(Mute::Until(until)) && c.marked_unread).map(|_| ())
    })
    .await;
    assert!(connection.opened(&id).await.unwrap().is_some_and(|c| !c.marked_unread));
    // Archived: when, in PingCraft's own room account data; and back.
    let archived = connection.archive(&id, true).await.unwrap();
    assert!(archived.archived.is_some() && archived.is_archived());
    inbox_until("the archived chat in the inbox", &mut feed, &mut inbox, |inbox| inbox.get(&id).filter(|c| c.is_archived()).map(|_| ())).await;
    assert_eq!(connection.archive(&id, false).await.unwrap().archived, None);
    // The chat's own sound and colour, in PingCraft's room account data.
    let alerts = pingcraft_core::Alerts { sound: Some(false), accent: Some(pingcraft_core::Accent::WhatsApp) };
    assert_eq!(connection.set_chat_alerts(&id, alerts).await.unwrap().alerts, alerts);
    inbox_until("the chat's sound and colour in the inbox", &mut feed, &mut inbox, |inbox| inbox.get(&id).filter(|c| c.alerts == alerts).map(|_| ()))
        .await;
    assert_eq!(connection.mute(&id, None).await.unwrap().muted, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn drafts_are_kept_and_typing_is_seen() {
    use pingcraft_core::ChatId;
    use pingcraft_matrix::ChatUpdate;
    let server = server_or_skip!();
    let ana = sign_in_as(&server.homeserver, &server.user).await;
    let ben = sign_in_as(&server.homeserver, &server.other).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();
    let room = create_room(&ana, &unique("Typing"), true, Some(ben_id), vec![]).await;
    join_and_say(&ben, &room, "here").await;
    let (ana_connection, _ana_chat) = open(&ana, &room).await;
    let id = ChatId(room.to_string());
    assert_eq!(ana_connection.save_draft(&id, "not sent yet", None).await.unwrap().draft.as_deref(), Some("not sent yet"));
    // A reply in progress is kept with it, by event ID (the store does not check the event exists).
    let reply = pingcraft_core::DraftFor::Reply("$somewhere:example.org".into());
    let kept = ana_connection.save_draft(&id, "about that", Some(&reply)).await.unwrap();
    assert_eq!((kept.draft.as_deref(), kept.draft_for), (Some("about that"), Some(reply)));
    assert_eq!(ana_connection.save_draft(&id, "", None).await.unwrap().draft, None);

    let (_ben_connection, mut ben_chat) = open(&ben, &room).await;
    ana_connection.typing(&id, true).await.unwrap();
    within("ana typing, seen by ben", async {
        loop {
            if let Some(ChatUpdate::Typing(names)) = ben_chat.next_update().await
                && !names.is_empty()
            {
                return;
            }
        }
    })
    .await;
    ana_connection.typing(&id, false).await.unwrap();
}

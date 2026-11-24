//! The inbox against a real homeserver: networks from bridge info, names, previews, unread counts and encryption.
//! See `common/mod.rs` for how to point it at one.

mod common;

use pingcraft_core::{ChatId, Inbox, MessageBody, Network};
use pingcraft_matrix::Connection;
use serde_json::json;

use common::{create_room, inbox_until, join_and_say, sign_in_as, state, unique, within};

#[tokio::test(flavor = "multi_thread")]
async fn bridged_chats_carry_their_network_preview_and_unread_count() {
    let server = server_or_skip!();
    let ana = within("ana signs in", sign_in_as(&server.homeserver, &server.user)).await;
    let ben = within("ben signs in", sign_in_as(&server.homeserver, &server.other)).await;
    let ben_id = ben.session.client.user_id().unwrap().to_owned();

    let connection = within("the sync to start", Connection::start(ana.session.client.clone())).await.unwrap();
    let mut feed = within("the inbox to open", connection.inbox()).await.unwrap();
    let mut inbox = Inbox::default();
    // The first answer: ana's keys are on the server before ben encrypts for her.
    inbox_until("the first inbox", &mut feed, &mut inbox, |_| Some(())).await;

    let name = unique("Bridged");
    // Ana stands in for the bridge bot: the event's sender (the room's creator) is the bot it names, on this server.
    let ana_id = ana.session.client.user_id().unwrap().to_string();
    let bridge = json!({"protocol": {"id": "telegram", "displayname": "Telegram"}, "bridgebot": ana_id});
    let room =
        within("the bridged room", create_room(&ana, &name, true, Some(ben_id), vec![state("m.bridge", "pingcraft.test://telegram/1", bridge)]))
            .await;
    within("ben to join and say hello", join_and_say(&ben, &room, "hello from ben")).await;

    let id = ChatId(room.to_string());
    let chat = inbox_until("the bridged chat with ben's message", &mut feed, &mut inbox, |inbox| {
        inbox.get(&id).filter(|c| c.last_message.as_ref().is_some_and(|m| m.body == MessageBody::Text("hello from ben".into()))).cloned()
    })
    .await;
    assert_eq!(chat.name, name);
    assert_eq!(chat.network, Network::from_id("telegram"));
    assert!(chat.encrypted);
    let preview = chat.last_message.unwrap();
    assert!(!preview.own);
    assert!(preview.at.0 > 0);
    let chat = inbox_until("one unread message", &mut feed, &mut inbox, |inbox| inbox.get(&id).filter(|c| c.unread > 0).cloned()).await;
    assert_eq!(chat.unread, 1, "one message from ben is one unread message");

    within("the sync to stop", connection.stop()).await;
    ana.sign_out().await;
    ben.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bridge_added_later_changes_the_network_and_an_empty_state_key_does_not() {
    let server = server_or_skip!();
    let ana = within("ana signs in", sign_in_as(&server.homeserver, &server.user)).await;

    // Bridge info with an empty state key is not a bridge's: the chat stays Matrix.
    let name = unique("Plain");
    let ignored = state("m.bridge", "", json!({"protocol": {"id": "discord"}}));
    let room = create_room(&ana, &name, false, None, vec![ignored]).await;

    let connection = within("the sync to start", Connection::start(ana.session.client.clone())).await.unwrap();
    let mut feed = within("the inbox to open", connection.inbox()).await.unwrap();
    let mut inbox = Inbox::default();
    let id = ChatId(room.to_string());
    let chat = inbox_until("the plain chat", &mut feed, &mut inbox, |inbox| inbox.get(&id).cloned()).await;
    assert!(chat.network.is_matrix(), "{:?}", chat.network);
    assert!(!chat.encrypted);

    // The older event type, added while the inbox is open. First one that names a bot other than its sender: it is
    // ignored. Then one whose sender is the bot it names (ana stands in for it).
    let matrix_room = ana.session.client.get_room(&room).unwrap();
    let spoof = json!({"protocol": {"id": "whatsapp"}, "bridgebot": "@whatsappbot:example.org"});
    matrix_room.send_state_event_raw("uk.half-shot.bridge", "pingcraft.test://a-spoof/1", spoof).await.unwrap();
    let ana_id = ana.session.client.user_id().unwrap().to_string();
    let content = json!({"protocol": {"id": "signal"}, "bridgebot": ana_id});
    matrix_room.send_state_event_raw("uk.half-shot.bridge", "pingcraft.test://signal/1", content).await.unwrap();
    let chat =
        inbox_until("the chat to move to Signal", &mut feed, &mut inbox, |inbox| inbox.get(&id).filter(|c| !c.network.is_matrix()).cloned()).await;
    assert_eq!(chat.network, Network::from_id("signal"), "the spoofed WhatsApp label was never shown");

    within("the sync to stop", connection.stop()).await;
    ana.sign_out().await;
}

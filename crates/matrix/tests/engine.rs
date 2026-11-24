//! The desktop engine against a real homeserver, through the same commands and events the interface uses: unlock
//! with a passphrase (as on a system without a keychain), sign in, see the inbox, read and send, come back after a
//! restart, and sign out. See `common/mod.rs` for how to point it at one.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use pingcraft_core::app::{Backend, Command, Event};
use pingcraft_core::{ChatId, InboxChange, MessageBody, SendState};
use pingcraft_keychain::KdfParams;
use pingcraft_matrix::engine::{Engine, Secrets};
use zeroize::Zeroizing;

use common::{PATIENCE, create_room, register_fresh_user, sign_in_as, unique};

const CHEAP: Secrets = Secrets::Passphrase(KdfParams { memory_kib: 8 * 1024, iterations: 1, lanes: 1 });

/// Polls the engine until `check` finds something in an event, keeping every event seen.
async fn until<T>(engine: &mut Engine, seen: &mut Vec<Event>, what: &str, check: impl Fn(&Event) -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        for event in engine.poll() {
            let found = check(&event);
            seen.push(event);
            if let Some(found) = found {
                return found;
            }
        }
        assert!(start.elapsed() < PATIENCE, "timed out waiting for {what}; saw {seen:?}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn start(data: &std::path::Path) -> Engine {
    Engine::start(data.to_owned(), CHEAP, Arc::new(|| {})).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn sign_in_read_send_restart_and_sign_out() {
    let server = server_or_skip!();
    let user = register_fresh_user(&server.homeserver).await.unwrap_or_else(|| server.user.clone());
    let data = tempfile::tempdir().unwrap();
    let mut seen = Vec::new();

    let mut engine = start(data.path());
    until(&mut engine, &mut seen, "the first unlock", |e| matches!(e, Event::Locked { first: true }).then_some(())).await;
    engine.send(Command::Unlock { passphrase: Zeroizing::new("a passphrase".into()) });
    until(&mut engine, &mut seen, "nobody signed in", |e| (*e == Event::SignedOut).then_some(())).await;

    engine.send(Command::SignIn { server: server.homeserver.clone(), username: user.0.clone(), password: Zeroizing::new(user.1.clone()) });
    let me = until(&mut engine, &mut seen, "signed in", |e| match e {
        Event::SignedIn { user } => Some(user.clone()),
        _ => None,
    })
    .await;
    assert!(me.starts_with(&format!("@{}:", user.0)), "{me}");

    // Another device of the same user makes a chat and says something in it.
    let other = sign_in_as(&server.homeserver, &user).await;
    let name = unique("Engine");
    let room = create_room(&other, &name, false, None, vec![]).await;
    other.session.client.get_room(&room).unwrap().send(RoomMessageEventContent::text_plain("from the other device")).await.unwrap();
    let chat = ChatId(room.to_string());
    until(&mut engine, &mut seen, "the chat in the inbox", |e| match e {
        Event::Inbox(InboxChange::Upsert(c)) if c.id == chat => Some(()),
        Event::Inbox(InboxChange::Reset(cs)) if cs.iter().any(|c| c.id == chat) => Some(()),
        _ => None,
    })
    .await;

    engine.send(Command::OpenChat(chat.clone()));
    until(&mut engine, &mut seen, "the message", |e| match e {
        Event::Messages { chat: c, messages }
            if *c == chat && messages.iter().any(|m| m.body == MessageBody::Text("from the other device".into())) =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    engine.send(Command::SendText { chat: chat.clone(), text: "from the engine".into() });
    until(&mut engine, &mut seen, "our message, sent", |e| match e {
        Event::Messages { messages, .. } => {
            messages.iter().any(|m| m.own && m.body == MessageBody::Text("from the engine".into()) && m.state == SendState::Sent).then_some(())
        }
        _ => None,
    })
    .await;

    // After a restart: locked again, and the same passphrase brings the same user back.
    drop(engine);
    let mut engine = start(data.path());
    until(&mut engine, &mut seen, "locked after the restart", |e| matches!(e, Event::Locked { first: false }).then_some(())).await;
    engine.send(Command::Unlock { passphrase: Zeroizing::new("not it".into()) });
    until(&mut engine, &mut seen, "a wrong passphrase", |e| (*e == Event::WrongPassphrase).then_some(())).await;
    engine.send(Command::Unlock { passphrase: Zeroizing::new("a passphrase".into()) });
    let again = until(&mut engine, &mut seen, "signed in again", |e| match e {
        Event::SignedIn { user } => Some(user.clone()),
        _ => None,
    })
    .await;
    assert_eq!(again, me);

    engine.send(Command::SignOut);
    until(&mut engine, &mut seen, "signed out", |e| (*e == Event::SignedOut).then_some(())).await;
    assert!(!data.path().join("profile").exists(), "nothing says who was signed in");
    other.sign_out().await;
}

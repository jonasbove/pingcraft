//! Shared by the tests that need a homeserver. They run against a throwaway one, given by:
//!
//! - `PINGCRAFT_TEST_HOMESERVER`: its address;
//! - `PINGCRAFT_TEST_USER` and `PINGCRAFT_TEST_PASSWORD`: a test user;
//! - `PINGCRAFT_TEST_OTHER_USER` and `PINGCRAFT_TEST_OTHER_PASSWORD`: a second test user, for chats between two;
//! - `PINGCRAFT_TEST_REGISTRATION_TOKEN` (optional): the server's registration token, for tests that need a new user
//!   each run (encryption, since a recovery key stays with an account once set up).
//!
//! Without them, each test says it was skipped and passes. Run them one at a time (`--test-threads 1`): they share
//! the users. Each test makes its own rooms, so a server can be reused between runs.
#![allow(dead_code)]

use std::future::Future;
use std::time::Duration;

use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::api::client::room::create_room::v3::Request as CreateRoom;
use matrix_sdk::ruma::events::AnyInitialStateEvent;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use pingcraft_keychain::{MemoryStore, new_profile_id};
use pingcraft_matrix::session::{Session, SignIn, sign_in};
use serde_json::json;
use zeroize::Zeroizing;

/// How long a test waits for the server to show something before it fails.
pub const PATIENCE: Duration = Duration::from_secs(60);

pub struct Server {
    pub homeserver: String,
    pub user: (String, String),
    pub other: (String, String),
}

pub fn server() -> Option<Server> {
    let var = |name: &str| std::env::var(name).ok();
    Some(Server {
        homeserver: var("PINGCRAFT_TEST_HOMESERVER")?,
        user: (var("PINGCRAFT_TEST_USER")?, var("PINGCRAFT_TEST_PASSWORD")?),
        other: (var("PINGCRAFT_TEST_OTHER_USER")?, var("PINGCRAFT_TEST_OTHER_PASSWORD")?),
    })
}

/// Returns early from a test that needs a server when none is given.
#[macro_export]
macro_rules! server_or_skip {
    () => {
        match common::server() {
            Some(s) => s,
            None => {
                eprintln!("skipped: PINGCRAFT_TEST_HOMESERVER and the test users are not set (see tests/common/mod.rs)");
                return;
            }
        }
    };
}

/// One signed-in device: its store folder and secrets live as long as this does.
pub struct Device {
    pub session: Session,
    pub secrets: MemoryStore,
    pub data: tempfile::TempDir,
}

pub async fn sign_in_as(server: &str, (user, password): &(String, String)) -> Device {
    let data = tempfile::tempdir().unwrap();
    let secrets = MemoryStore::default();
    let request = SignIn { server: server.to_owned(), username: user.clone(), password: Zeroizing::new(password.clone()) };
    let session = sign_in(data.path(), &secrets, &request).await.unwrap();
    Device { session, secrets, data }
}

impl Device {
    /// Signs the device out, so test devices do not pile up on the server.
    pub async fn sign_out(self) {
        pingcraft_matrix::session::sign_out(self.data.path(), &self.secrets, self.session).await.unwrap();
    }
}

/// Waits until a chat's list of messages passes `check`, and returns what it found.
pub async fn chat_until<T>(
    what: &str,
    chat: &mut pingcraft_matrix::ChatTimeline,
    last: &mut Vec<pingcraft_core::Message>,
    check: impl Fn(&[pingcraft_core::Message]) -> Option<T>,
) -> T {
    within(what, async {
        loop {
            if let Some(found) = check(last) {
                return found;
            }
            *last = chat.next().await.expect("the chat closed");
        }
    })
    .await
}

/// Registers a new user with the server's registration token, and returns its name and password. `None` when no
/// token is given.
pub async fn register_fresh_user(homeserver: &str) -> Option<(String, String)> {
    let token = std::env::var("PINGCRAFT_TEST_REGISTRATION_TOKEN").ok()?;
    let id = new_profile_id().unwrap();
    let (user, password) = (format!("test-{}", &id[..10]), format!("pw-{}", &id[10..]));
    let url = format!("{}/_matrix/client/v3/register", homeserver.trim_end_matches('/'));
    let http = matrix_sdk::reqwest::Client::new();
    let post = |body: serde_json::Value| http.post(&url).header("content-type", "application/json").body(body.to_string()).send();
    // The first request only starts the interactive auth and gives its session.
    let started: String = post(json!({"username": user, "password": password})).await.unwrap().text().await.unwrap();
    let started: serde_json::Value = serde_json::from_str(&started).unwrap();
    let session = started["session"].as_str().expect("the server offers interactive registration").to_owned();
    let auth = json!({"type": "m.login.registration_token", "token": token, "session": session});
    let done = post(json!({"username": user, "password": password, "inhibit_login": true, "auth": auth})).await.unwrap();
    assert!(done.status().is_success(), "registration failed: {}", done.status());
    Some((user, password))
}

/// A name no other test run has used.
pub fn unique(prefix: &str) -> String {
    format!("{prefix} {}", &new_profile_id().unwrap()[..8])
}

/// Runs `wait`, failing the test if it takes longer than [`PATIENCE`].
pub async fn within<T>(what: &str, wait: impl Future<Output = T>) -> T {
    let started = std::time::Instant::now();
    match tokio::time::timeout(PATIENCE, wait).await {
        Ok(found) => {
            eprintln!("{what}: {:.1?}", started.elapsed());
            found
        }
        Err(_) => panic!("timed out waiting for {what}"),
    }
}

/// Applies inbox changes until `check` finds what it looks for.
pub async fn inbox_until<T>(
    what: &str,
    feed: &mut pingcraft_matrix::InboxFeed,
    inbox: &mut pingcraft_core::Inbox,
    check: impl Fn(&pingcraft_core::Inbox) -> Option<T>,
) -> T {
    within(what, async {
        loop {
            if let Some(found) = check(inbox) {
                return found;
            }
            let change = feed.next().await.expect("the inbox feed ended");
            inbox.apply(change);
        }
    })
    .await
}

pub fn state(event_type: &str, state_key: &str, content: serde_json::Value) -> Raw<AnyInitialStateEvent> {
    Raw::from_json_string(json!({"type": event_type, "state_key": state_key, "content": content}).to_string()).unwrap()
}

/// Creates a room as `device`, inviting `invite`, with extra initial state.
pub async fn create_room(
    device: &Device,
    name: &str,
    encrypted: bool,
    invite: Option<OwnedUserId>,
    mut initial: Vec<Raw<AnyInitialStateEvent>>,
) -> OwnedRoomId {
    if encrypted {
        initial.push(state("m.room.encryption", "", json!({"algorithm": "m.megolm.v1.aes-sha2"})));
    }
    let mut request = CreateRoom::new();
    request.name = Some(name.to_owned());
    request.initial_state = initial;
    request.invite = invite.into_iter().collect();
    device.session.client.create_room(request).await.unwrap().room_id().to_owned()
}

/// Joins `room` as `device` and sends a text message into it.
pub async fn join_and_say(device: &Device, room: &OwnedRoomId, text: &str) {
    let client = &device.session.client;
    client.join_room_by_id(room).await.unwrap();
    client.sync_once(SyncSettings::default()).await.unwrap();
    let room = client.get_room(room).unwrap();
    room.send(RoomMessageEventContent::text_plain(text)).await.unwrap();
}

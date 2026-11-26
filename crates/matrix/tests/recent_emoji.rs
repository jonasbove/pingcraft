//! The account's recently used emoji (`m.recent_emoji`) against a real homeserver: they follow the user to another
//! device, and a device's uses are merged into what another device saved, not written over it. See
//! `common/mod.rs` for how to point it at one.

mod common;

use std::time::Instant;

use pingcraft_matrix::Connection;
use pingcraft_matrix::recent_emoji::Pending;

use common::{sign_in_as, within};

/// Waits until `connection`'s synced copy of the recently used emoji starts with `first`.
async fn synced_until(connection: &Connection, first: &[&str]) -> Vec<String> {
    within("the recently used emoji to sync", async {
        loop {
            let (emoji, _) = connection.recent_emoji(&Pending::default()).await;
            if emoji.iter().map(String::as_str).take(first.len()).eq(first.iter().copied()) {
                return emoji;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn recent_emoji_follow_the_user_between_devices_and_merge() {
    let server = server_or_skip!();
    // The same user on two devices. The account may have recently used emoji from earlier runs; only the front of
    // the list is checked.
    let laptop = sign_in_as(&server.homeserver, &server.user).await;
    let phone = sign_in_as(&server.homeserver, &server.user).await;
    let on_laptop = within("the laptop's sync to start", Connection::start(laptop.session.client.clone())).await.unwrap();
    let on_phone = within("the phone's sync to start", Connection::start(phone.session.client.clone())).await.unwrap();

    let mut laptop_uses = Pending::default();
    laptop_uses.used("👍".to_owned(), Instant::now());
    laptop_uses.used("🧪".to_owned(), Instant::now());
    let saved = within("saving on the laptop", on_laptop.save_recent_emoji(&mut laptop_uses)).await.unwrap();
    assert_eq!(saved[..2], ["🧪", "👍"]);
    assert!(!laptop_uses.due(None), "sent, so nothing is pending");

    // The phone sees them once it has synced.
    synced_until(&on_phone, &["🧪", "👍"]).await;

    // The phone uses one; then the laptop, before it has synced that, uses another. Its save builds on what the
    // server has now, so the phone's is kept.
    let mut phone_uses = Pending::default();
    phone_uses.used("🎻".to_owned(), Instant::now());
    within("saving on the phone", on_phone.save_recent_emoji(&mut phone_uses)).await.unwrap();
    laptop_uses.used("👍".to_owned(), Instant::now());
    let saved = within("saving on the laptop again", on_laptop.save_recent_emoji(&mut laptop_uses)).await.unwrap();
    assert_eq!(saved[..3], ["👍", "🎻", "🧪"], "the phone's 🎻 is merged in, not written over");
    synced_until(&on_phone, &["👍", "🎻", "🧪"]).await;

    on_laptop.stop().await;
    on_phone.stop().await;
    laptop.sign_out().await;
    phone.sign_out().await;
}

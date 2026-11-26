//! The emoji skin tone kept with the account (`io.github.jonasbove.pingcraft.emoji`) against a real homeserver: a
//! tone chosen on one device is what the server then says, and reaches another device through the sync. See
//! `common/mod.rs` for how to point it at one.

mod common;

use pingcraft_matrix::Connection;

use common::{sign_in_as, within};

/// Waits until `connection`'s synced copy of the skin tone is `tone`.
async fn synced_until(connection: &Connection, tone: &str) {
    within("the skin tone to sync", async {
        while connection.synced_skin_tone().await.as_deref() != Some(tone) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_skin_tone_follows_the_user_between_devices() {
    let server = server_or_skip!();
    // The same user on two devices. The account may keep a tone from an earlier run, so the tones set here are
    // chosen to differ from it.
    let laptop = sign_in_as(&server.homeserver, &server.user).await;
    let phone = sign_in_as(&server.homeserver, &server.user).await;
    let on_laptop = within("the laptop's sync to start", Connection::start(laptop.session.client.clone())).await.unwrap();
    let on_phone = within("the phone's sync to start", Connection::start(phone.session.client.clone())).await.unwrap();

    let before = within("asking the server", on_laptop.fetch_skin_tone()).await.unwrap();
    let first = if before.as_deref() == Some("dark") { "light" } else { "dark" };
    let second = if first == "dark" { "medium" } else { "medium-light" };

    within("saving on the laptop", on_laptop.save_skin_tone(first)).await.unwrap();
    assert_eq!(within("asking the server again", on_phone.fetch_skin_tone()).await.unwrap().as_deref(), Some(first));
    synced_until(&on_phone, first).await;

    // The phone changes it; the laptop sees that through its sync.
    within("saving on the phone", on_phone.save_skin_tone(second)).await.unwrap();
    synced_until(&on_laptop, second).await;

    on_laptop.stop().await;
    on_phone.stop().await;
    laptop.sign_out().await;
    phone.sign_out().await;
}

//! Whether notifications show message text, kept with the account (`io.github.jonasbove.pingcraft.notifications`)
//! against a real homeserver: set on one device, what the server then says, and seen on another through the sync.
//! See `common/mod.rs` for how to point it at one.

mod common;

use pingcraft_matrix::Connection;

use common::{sign_in_as, within};

async fn synced_until(connection: &Connection, on: bool) {
    within("the previews setting to sync", async {
        while connection.synced_previews().await != Some(on) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_previews_setting_follows_the_user_between_devices() {
    let server = server_or_skip!();
    let laptop = sign_in_as(&server.homeserver, &server.user).await;
    let phone = sign_in_as(&server.homeserver, &server.user).await;
    let on_laptop = within("the laptop's sync to start", Connection::start(laptop.session.client.clone())).await.unwrap();
    let on_phone = within("the phone's sync to start", Connection::start(phone.session.client.clone())).await.unwrap();

    // The account may keep a setting from an earlier run: this changes it.
    let before = within("asking the server", on_laptop.fetch_previews()).await.unwrap();
    let first = before == Some(false);
    within("saving on the laptop", on_laptop.save_previews(first)).await.unwrap();
    assert_eq!(within("asking the server again", on_phone.fetch_previews()).await.unwrap(), Some(first));
    synced_until(&on_phone, first).await;
    within("saving on the phone", on_phone.save_previews(!first)).await.unwrap();
    synced_until(&on_laptop, !first).await;

    on_laptop.stop().await;
    on_phone.stop().await;
    laptop.sign_out().await;
    phone.sign_out().await;
}

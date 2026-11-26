//! Each network's notification sound and accent colour (`io.github.jonasbove.pingcraft.networks`) against a real
//! homeserver: set on one device, seen on another through the sync, with the other networks' settings kept. See
//! `common/mod.rs` for how to point it at one.

mod common;

use pingcraft_core::{Accent, Alerts, Network};
use pingcraft_matrix::Connection;

use common::{sign_in_as, within};

/// Waits until `connection`'s synced settings for `network` are `alerts`.
async fn synced_until(connection: &Connection, network: &Network, alerts: Alerts) {
    within("the network's settings to sync", async {
        while connection.network_alerts().await.0.get(network).copied().unwrap_or_default() != alerts {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn network_alerts_follow_the_user_between_devices() {
    let server = server_or_skip!();
    let laptop = sign_in_as(&server.homeserver, &server.user).await;
    let phone = sign_in_as(&server.homeserver, &server.user).await;
    let on_laptop = within("the laptop's sync to start", Connection::start(laptop.session.client.clone())).await.unwrap();
    let on_phone = within("the phone's sync to start", Connection::start(phone.session.client.clone())).await.unwrap();
    let (telegram, signal) = (Network::from_id("telegram"), Network::from_id("signal"));
    // The account may keep settings from an earlier run: these differ from them.
    let before = on_laptop.network_alerts().await.0.get(&telegram).copied().unwrap_or_default();
    let one = Alerts { sound: Some(false), accent: Some(Accent::Discord) };
    let quiet = if before == one { Alerts { sound: Some(true), accent: Some(Accent::Signal) } } else { one };

    within("saving on the laptop", on_laptop.set_network_alerts(&telegram, quiet)).await.unwrap();
    synced_until(&on_phone, &telegram, quiet).await;
    let signal_before = on_laptop.network_alerts().await.0.get(&signal).copied().unwrap_or_default();
    let blue = Alerts { sound: None, accent: Some(if signal_before.accent == Some(Accent::Telegram) { Accent::WhatsApp } else { Accent::Telegram }) };
    within("saving on the phone", on_phone.set_network_alerts(&signal, blue)).await.unwrap();
    synced_until(&on_laptop, &signal, blue).await;
    synced_until(&on_laptop, &telegram, quiet).await;

    on_laptop.stop().await;
    on_phone.stop().await;
    laptop.sign_out().await;
    phone.sign_out().await;
}

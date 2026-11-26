//! Encryption against a real homeserver: the recovery key on first sign-in, recovering on a new device, and
//! verifying a new device from an old one by comparing symbols. Each run registers a new user, since a recovery
//! key, once set up, stays with the account: this needs `PINGCRAFT_TEST_REGISTRATION_TOKEN` as well (see
//! `common/mod.rs`).

mod common;

use pingcraft_core::encryption::{RecoveryKey, RecoveryStatus, Verification};
use pingcraft_matrix::Connection;
use pingcraft_matrix::encryption::{EncryptionError, VerificationFlow};

use common::{register_fresh_user, sign_in_as, within};

async fn connect(device: &common::Device) -> Connection {
    within("the sync to start", Connection::start(device.session.client.clone())).await.unwrap()
}

/// Waits until `known_by` has seen `device`'s keys: a request from a device the other does not know yet is
/// dropped there.
async fn wait_until_known(known_by: &Connection, device: &common::Device) {
    let client = &device.session.client;
    let (user, id) = (client.user_id().unwrap().to_owned(), client.device_id().unwrap().to_owned());
    within("the old device to see the new one", async {
        while !matches!(known_by.client().encryption().get_device(&user, &id).await, Ok(Some(_))) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await;
}

/// Follows a flow until `stop` matches, returning that state.
async fn flow_until(what: &str, flow: &mut VerificationFlow, stop: impl Fn(&Verification) -> bool) -> Verification {
    within(what, async {
        loop {
            let state = flow.next().await.expect("the verification ended");
            if stop(&state) {
                return state;
            }
            assert!(!matches!(state, Verification::Cancelled(_)), "cancelled while waiting for {what}: {state:?}");
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn first_sign_in_sets_up_a_recovery_key_a_new_device_recovers_and_another_is_verified_by_symbols() {
    let server = server_or_skip!();
    let Some(user) = register_fresh_user(&server.homeserver).await else {
        eprintln!("skipped: PINGCRAFT_TEST_REGISTRATION_TOKEN is not set");
        return;
    };

    // The first device sets up cross-signing and a recovery key.
    let first = sign_in_as(&server.homeserver, &user).await;
    let first_sync = connect(&first).await;
    let key = within("encryption to be set up", first_sync.set_up_encryption(Some(&user.1))).await.unwrap().expect("a new recovery key");
    assert_eq!(key.groups().len(), 12, "a recovery key is 48 characters in groups of four");
    assert_eq!(first_sync.recovery_status(), RecoveryStatus::On);
    // The device's own signature shows once the keys are fetched back from the server.
    within("the first device to be cross-signed", async {
        while !first_sync.is_this_device_verified().await {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await;
    // Setting up again creates nothing new.
    assert!(first_sync.set_up_encryption(Some(&user.1)).await.unwrap().is_none());

    // A second device finds the backup, refuses to replace it, and recovers with the key.
    let second = sign_in_as(&server.homeserver, &user).await;
    let second_sync = connect(&second).await;
    assert_eq!(within("the recovery state", second_sync.known_recovery_status()).await, RecoveryStatus::NeedsKey);
    assert!(matches!(second_sync.set_up_encryption(Some(&user.1)).await, Err(EncryptionError::BackupExists)));
    let wrong = RecoveryKey::new("EsTa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa aaaa");
    assert!(second_sync.recover(&wrong).await.is_err(), "a wrong key does not recover");
    within("recovering with the key", second_sync.recover(&key)).await.unwrap();
    assert_eq!(second_sync.recovery_status(), RecoveryStatus::On);

    // A third device is verified from the first by comparing symbols.
    let third = sign_in_as(&server.homeserver, &user).await;
    let third_sync = connect(&third).await;
    wait_until_known(&first_sync, &third).await;
    let mut incoming = first_sync.incoming_verifications();
    let mut asking = within("the request to be sent", third_sync.verify_this_device()).await.unwrap();
    let mut answering = within("the request to arrive", incoming.next()).await.unwrap();
    flow_until("the request to show", &mut answering, |s| matches!(s, Verification::Incoming { .. })).await;
    answering.accept().await.unwrap();

    let Verification::Compare(shown_there) = flow_until("symbols on the new device", &mut asking, |s| matches!(s, Verification::Compare(_))).await
    else {
        unreachable!()
    };
    let Verification::Compare(shown_here) = flow_until("symbols on the old device", &mut answering, |s| matches!(s, Verification::Compare(_))).await
    else {
        unreachable!()
    };
    assert_eq!(shown_there.len(), 7);
    assert_eq!(shown_there, shown_here, "both devices show the same symbols");
    assert!(shown_there.iter().all(|s| s.index < 64 && !s.symbol.is_empty() && !s.name.is_empty()));

    answering.confirm().await.unwrap();
    asking.confirm().await.unwrap();
    flow_until("the old device to finish", &mut answering, |s| *s == Verification::Done).await;
    flow_until("the new device to finish", &mut asking, |s| *s == Verification::Done).await;
    within("the new device to be cross-signed", async {
        while !third_sync.is_this_device_verified().await {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await;

    for (sync, device) in [(first_sync, first), (second_sync, second), (third_sync, third)] {
        sync.stop().await;
        device.sign_out().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn saying_the_symbols_do_not_match_trusts_nothing() {
    let server = server_or_skip!();
    let Some(user) = register_fresh_user(&server.homeserver).await else {
        eprintln!("skipped: PINGCRAFT_TEST_REGISTRATION_TOKEN is not set");
        return;
    };
    let first = sign_in_as(&server.homeserver, &user).await;
    let first_sync = connect(&first).await;
    within("encryption to be set up", first_sync.set_up_encryption(Some(&user.1))).await.unwrap();
    let second = sign_in_as(&server.homeserver, &user).await;
    let second_sync = connect(&second).await;
    wait_until_known(&first_sync, &second).await;

    let mut incoming = first_sync.incoming_verifications();
    let mut asking = within("the request to be sent", second_sync.verify_this_device()).await.unwrap();
    let mut answering = within("the request to arrive", incoming.next()).await.unwrap();
    answering.accept().await.unwrap();
    flow_until("symbols on the new device", &mut asking, |s| matches!(s, Verification::Compare(_))).await;
    flow_until("symbols on the old device", &mut answering, |s| matches!(s, Verification::Compare(_))).await;

    asking.mismatch().await.unwrap();
    let there = within("the new device to stop", async {
        loop {
            match asking.next().await {
                Some(s @ Verification::Cancelled(_)) => return s,
                Some(_) => continue,
                None => panic!("ended without a state"),
            }
        }
    })
    .await;
    assert_eq!(there, Verification::Cancelled(pingcraft_core::encryption::CancelReason::Mismatch));
    assert!(!second_sync.is_this_device_verified().await, "nothing was trusted");

    for (sync, device) in [(first_sync, first), (second_sync, second)] {
        sync.stop().await;
        device.sign_out().await;
    }
}

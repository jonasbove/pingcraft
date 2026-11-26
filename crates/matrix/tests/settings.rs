//! Settings' Security part against a real homeserver: the device list, renaming this device, signing another out
//! with the account password, verifying another device from the list, and making a new recovery key. See
//! `common/mod.rs` for how to point it at one.
//!
//! What each test changes on the server: `devices_are_listed_renamed_and_signed_out` signs the test user in on two
//! devices it makes itself, renames the first and signs the second out (never a device it did not make); the
//! recovery key tests register a new user each run (`PINGCRAFT_TEST_REGISTRATION_TOKEN`), so the shared test user
//! never gets a recovery key from them.

mod common;

use pingcraft_core::devices::{DeviceId, SignOutProblem};
use pingcraft_core::encryption::{RecoveryKey, RecoveryStatus, Verification};
use pingcraft_matrix::Connection;
use pingcraft_matrix::encryption::EncryptionError;

use common::{register_fresh_user, sign_in_as, unique, within};

async fn connect(device: &common::Device) -> Connection {
    within("the sync to start", Connection::start(device.session.client.clone())).await.unwrap()
}

fn id_of(device: &common::Device) -> DeviceId {
    DeviceId(device.session.client.device_id().unwrap().to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_are_listed_renamed_and_signed_out() {
    let server = server_or_skip!();
    let laptop = sign_in_as(&server.homeserver, &server.user).await;
    let phone = sign_in_as(&server.homeserver, &server.user).await;
    let on_laptop = connect(&laptop).await;
    let on_phone = connect(&phone).await;
    let (laptop_id, phone_id) = (id_of(&laptop), id_of(&phone));

    // Both are listed, this one first and marked as this one.
    let listed = within("the device list", on_laptop.devices()).await.unwrap();
    assert_eq!(listed.first().map(|d| (&d.id, d.this)), Some((&laptop_id, true)), "{listed:?}");
    assert!(listed.iter().any(|d| d.id == phone_id && !d.this), "{listed:?}");
    assert_eq!(listed.iter().filter(|d| d.this).count(), 1);

    // Renamed here, seen there.
    let name = unique("PingCraft test laptop");
    within("renaming", on_laptop.rename_this_device(&format!("  {name}  "))).await.unwrap();
    within("the new name on the other device", async {
        while !on_phone.devices().await.unwrap().iter().any(|d| d.id == laptop_id && d.name == name) {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await;
    assert!(on_laptop.rename_this_device("   ").await.is_err(), "an empty name is refused");

    // This device is not signed out this way; the other is, with the account password and not without it.
    assert!(matches!(on_laptop.sign_out_device(&laptop_id, &server.user.1).await, Err(SignOutProblem::Other(_))));
    assert_eq!(on_laptop.sign_out_device(&phone_id, "not the password").await, Err(SignOutProblem::WrongPassword));
    assert!(on_laptop.devices().await.unwrap().iter().any(|d| d.id == phone_id), "a refused sign-out leaves it");
    within("signing the other device out", on_laptop.sign_out_device(&phone_id, &server.user.1)).await.unwrap();
    assert!(!on_laptop.devices().await.unwrap().iter().any(|d| d.id == phone_id), "gone from the list");

    // The phone's session ended on the server, so it is only dropped here.
    on_phone.stop().await;
    drop(phone);
    on_laptop.stop().await;
    laptop.sign_out().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_recovery_key_replaces_the_old_one_and_another_device_is_asked_to_verify() {
    let server = server_or_skip!();
    let Some(user) = register_fresh_user(&server.homeserver).await else {
        eprintln!("skipped: PINGCRAFT_TEST_REGISTRATION_TOKEN is not set");
        return;
    };
    let first = sign_in_as(&server.homeserver, &user).await;
    let first_sync = connect(&first).await;
    let old = within("encryption to be set up", first_sync.set_up_encryption(Some(&user.1))).await.unwrap().expect("a first recovery key");
    let new = within("a new recovery key", first_sync.new_recovery_key()).await.unwrap();
    assert_ne!(new.expose(), old.expose());
    assert_eq!(new.groups().len(), 12);
    assert_eq!(first_sync.recovery_status(), RecoveryStatus::On, "the backup stays");

    // A second device opens the backup with the new key, not the old one, and cannot make a key of its own before.
    let second = sign_in_as(&server.homeserver, &user).await;
    let second_sync = connect(&second).await;
    assert_eq!(within("the recovery state", second_sync.known_recovery_status()).await, RecoveryStatus::NeedsKey);
    assert!(matches!(second_sync.new_recovery_key().await, Err(EncryptionError::BackupExists)), "a backup it cannot open is never replaced");
    assert!(second_sync.recover(&RecoveryKey::new(old.expose())).await.is_err(), "the old key stopped working");
    within("recovering with the new key", second_sync.recover(&new)).await.unwrap();

    // The first device asks the second, from the device list, to verify.
    let second_id = id_of(&second);
    let mut incoming = second_sync.incoming_verifications();
    let mut asking = within("the request to be sent", async {
        loop {
            match first_sync.verify_device(&second_id).await {
                Ok(flow) => return flow,
                // Until the first device has fetched the second's keys.
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
            }
        }
    })
    .await;
    let mut answering = within("the request to arrive", incoming.next()).await.unwrap();
    within("the request to show", async {
        loop {
            match answering.next().await {
                Some(Verification::Incoming { .. }) => break,
                Some(_) => {}
                None => panic!("the request ended before it showed"),
            }
        }
    })
    .await;
    asking.cancel().await.unwrap();

    for (sync, device) in [(first_sync, first), (second_sync, second)] {
        sync.stop().await;
        device.sign_out().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_recovery_key_can_be_made_from_settings() {
    let server = server_or_skip!();
    let Some(user) = register_fresh_user(&server.homeserver).await else {
        eprintln!("skipped: PINGCRAFT_TEST_REGISTRATION_TOKEN is not set");
        return;
    };
    let device = sign_in_as(&server.homeserver, &user).await;
    let sync = connect(&device).await;
    assert_eq!(within("the recovery state", sync.known_recovery_status()).await, RecoveryStatus::Off);
    let key = within("the first recovery key", sync.new_recovery_key()).await.unwrap();
    assert_eq!(key.groups().len(), 12);
    assert_eq!(sync.recovery_status(), RecoveryStatus::On);
    sync.stop().await;
    device.sign_out().await;
}

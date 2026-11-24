//! The mautrix provider against the real bridges of a throwaway stack from `deploy/`, through its proxy, with OpenID.
//! See `common/mod.rs` for the server; it also needs `PINGCRAFT_TEST_BRIDGES=1`, and the test user must have no
//! accounts on the bridges.
//!
//! Every login stops at its first step (a QR code, or a question for a phone number) and is cancelled: this never
//! logs in to a network. Nothing secret is printed: not the token, not the QR code's content.

mod common;

use std::time::Duration;

use pingcraft_core::Network;
use pingcraft_core::app::{Backend, Command, Event};
use pingcraft_core::bridge::{BridgeError, BridgeProvider, Display, FieldKind, LoginStep};
use pingcraft_keychain::KdfParams;
use pingcraft_matrix::engine::{Engine, Secrets};
use pingcraft_matrix::mautrix::MautrixProvider;
use zeroize::Zeroizing;

use common::{sign_in_as, within};

fn bridges_or_skip() -> bool {
    let on = std::env::var("PINGCRAFT_TEST_BRIDGES").is_ok_and(|v| v == "1");
    if !on {
        eprintln!("skipped: PINGCRAFT_TEST_BRIDGES=1 is not set");
    }
    on
}

/// What a first step is, without its content.
fn shape(step: &LoginStep) -> String {
    match step {
        LoginStep::Show { display: Display::Qr(data), .. } => format!("a QR code ({} characters)", data.len()),
        LoginStep::Show { display, .. } => format!("shows {display:?}"),
        LoginStep::Ask { fields, .. } => format!("asks for {:?}", fields.iter().map(|f| &f.kind).collect::<Vec<_>>()),
        LoginStep::Unsupported { kind, .. } => format!("unsupported {kind}"),
        LoginStep::Done { .. } => "done".into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn each_bridge_answers_and_a_login_stops_at_its_first_step() {
    let server = server_or_skip!();
    if !bridges_or_skip() {
        return;
    }
    let me = within("signing in", sign_in_as(&server.homeserver, &server.user)).await;
    let provider = MautrixProvider::for_client(&me.session.client).unwrap();

    for network in provider.networks() {
        let found = within("whoami", provider.network(&network)).await.unwrap_or_else(|e| panic!("{}: {e}", network.id()));
        eprintln!("{}: flows {:?}, {} accounts", network.id(), found.flows.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), found.accounts.len());
        assert!(!found.flows.is_empty());
        assert!(found.accounts.is_empty(), "the test user must have no accounts on {}", network.id());
    }

    // The first step of each, then cancelled.
    for (network, flow) in [("signal", "qr"), ("whatsapp", "qr"), ("telegram", "phone"), ("whatsapp", "phone")] {
        let network = Network::from_id(network);
        let first = within("the first step", provider.start_login(&network, flow)).await;
        let first = first.unwrap_or_else(|e| panic!("{} {flow}: {e}", network.id()));
        eprintln!("{} {flow}: {}", network.id(), shape(&first));
        match flow {
            "qr" => assert!(matches!(first, LoginStep::Show { display: Display::Qr(_), .. }), "{}", shape(&first)),
            _ => assert!(
                matches!(&first, LoginStep::Ask { fields, .. } if fields.iter().any(|f| f.kind == FieldKind::PhoneNumber)),
                "{}",
                shape(&first)
            ),
        }
        within("cancelling", provider.cancel(first.step().unwrap())).await.unwrap();
    }

    // Waiting on a QR code ends when the login is cancelled.
    let signal = Network::from_id("signal");
    let first = within("the Signal QR code", provider.start_login(&signal, "qr")).await.unwrap();
    let step = first.step().unwrap().clone();
    let waiting = {
        let provider = MautrixProvider::for_client(&me.session.client).unwrap();
        let step = step.clone();
        tokio::spawn(async move { provider.wait(&step).await })
    };
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!waiting.is_finished(), "nobody scanned the code, so it is still waiting");
    within("cancelling", provider.cancel(&step)).await.unwrap();
    let ended = within("the wait to end", waiting).await.unwrap();
    eprintln!("signal wait after cancel: {:?}", ended.as_ref().map(shape));
    assert!(matches!(ended, Err(BridgeError::LoginGone) | Err(BridgeError::Refused(_))), "{:?}", ended.map(|s| shape(&s)));
    assert_eq!(provider.cancel(&step).await, Ok(()), "cancelling twice is fine");

    for network in provider.networks() {
        let after = provider.network(&network).await.unwrap();
        assert!(after.accounts.is_empty(), "no account was made on {}", network.id());
    }
    me.sign_out().await;
}

/// Polls the engine until an event passes `check`, and returns it. Events before it are dropped; those after it
/// wait in `later` for the next call.
async fn until(engine: &mut Engine, later: &mut Vec<Event>, what: &str, check: impl Fn(&Event) -> bool) -> Event {
    let start = std::time::Instant::now();
    loop {
        later.extend(engine.poll());
        if let Some(at) = later.iter().position(&check) {
            let found = later.remove(at);
            later.drain(..at);
            return found;
        }
        // A failure says why at once, instead of waiting out the time.
        if let Some(failed) = later.iter().find(|e| matches!(e, Event::SignInFailed(_) | Event::Problem(_))) {
            panic!("waiting for {what}: {failed:?}");
        }
        assert!(start.elapsed() < common::PATIENCE, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The same through the engine, with the commands and events the interface uses.
#[tokio::test(flavor = "multi_thread")]
async fn the_engine_lists_networks_and_shows_a_qr_code_until_cancelled() {
    let server = server_or_skip!();
    if !bridges_or_skip() {
        return;
    }
    let data = tempfile::tempdir().unwrap();
    let cheap = Secrets::Passphrase(KdfParams { memory_kib: 8 * 1024, iterations: 1, lanes: 1 });
    let mut engine = Engine::start(data.path().to_owned(), cheap, std::sync::Arc::new(|| {})).unwrap();
    let mut later = Vec::new();
    until(&mut engine, &mut later, "the unlock", |e| matches!(e, Event::Locked { .. })).await;
    engine.send(Command::Unlock { passphrase: Zeroizing::new("p".into()) });
    until(&mut engine, &mut later, "signed out", |e| *e == Event::SignedOut).await;
    let (user, password) = server.user.clone();
    engine.send(Command::SignIn { server: server.homeserver.clone(), username: user, password: Zeroizing::new(password) });
    until(&mut engine, &mut later, "signed in", |e| matches!(e, Event::SignedIn { .. })).await;

    // Each network's answer comes as its own event, in any order.
    engine.send(Command::LoadNetworks);
    let mut answered = Vec::new();
    while answered.len() < 3 {
        let event = until(&mut engine, &mut later, "the networks", |e| matches!(e, Event::Network { .. })).await;
        let Event::Network { network, found } = event else { unreachable!() };
        assert!(found.is_ok(), "{}: {found:?}", network.id());
        answered.push(network);
    }

    engine.send(Command::StartLogin { network: Network::from_id("signal"), flow: "qr".into() });
    let shown = until(&mut engine, &mut later, "the QR code", |e| matches!(e, Event::Login(_) | Event::LoginFailed { .. })).await;
    let Event::Login(LoginStep::Show { step, display: Display::Qr(_), .. }) = shown else { panic!("not a QR code: {shown:?}") };
    engine.send(Command::CancelLogin(step));
    engine.send(Command::SignOut);
    until(&mut engine, &mut later, "signed out again", |e| *e == Event::SignedOut).await;
}

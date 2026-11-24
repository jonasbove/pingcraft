//! The mapping from the provisioning API's answers, and the requests sent, against a fake bridge on a local port.

use std::sync::{Arc, Mutex};

use pingcraft_core::bridge::{Answers, BoxFuture, BridgeError, BridgeProvider, Display, FieldKind, LoginStep, StepRef};
use pingcraft_core::{AccountId, AccountState, Network};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zeroize::Zeroizing;

use super::{MautrixProvider, TokenSource, account_state, bridged_network, error_for, escape, login_step};

fn signal() -> Network {
    Network::from_id("signal")
}

#[test]
fn a_qr_code_is_shown_and_waited_on() {
    let answer = json!({"login_id": "l1", "type": "display_and_wait", "step_id": "fi.mau.signal.qr", "txn_id": "t1", "instructions": "Scan",
        "display_and_wait": {"type": "qr", "data": "sgnl://linkdevice?uuid=x"}});
    let step = login_step(&signal(), &answer).unwrap();
    let expected = StepRef { network: signal(), login: "l1".into(), step: "fi.mau.signal.qr".into(), attempt: "t1".into() };
    assert_eq!(
        step,
        LoginStep::Show { step: expected, instructions: "Scan".into(), display: Display::Qr("sgnl://linkdevice?uuid=x".into()), can_cancel: false }
    );
}

#[test]
fn a_phone_number_is_asked_for() {
    let answer = json!({"login_id": "l", "type": "user_input", "step_id": "s", "txn_id": "t",
        "user_input": {"fields": [{"type": "phone_number", "id": "phone", "name": "Phone number", "description": "With the country code", "default_value": ""}], "can_cancel": true}});
    let LoginStep::Ask { fields, can_cancel, .. } = login_step(&Network::from_id("telegram"), &answer).unwrap() else { panic!("not a question") };
    assert!(can_cancel);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].kind, FieldKind::PhoneNumber);
    assert_eq!((fields[0].id.as_str(), fields[0].name.as_str(), fields[0].default.as_deref()), ("phone", "Phone number", None));
}

#[test]
fn steps_pingcraft_cannot_do_are_unsupported_not_errors() {
    let base = json!({"login_id": "l", "step_id": "s", "txn_id": "t"});
    let with = |extra: Value| {
        let mut v = base.clone();
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        login_step(&signal(), &v).unwrap()
    };
    for (extra, kind) in [
        (json!({"type": "cookies", "cookies": {"url": "https://example.org"}}), "cookies"),
        (json!({"type": "webauthn"}), "webauthn"),
        (json!({"type": "something_new"}), "something_new"),
        (json!({"type": "display_and_wait", "display_and_wait": {"type": "hologram", "data": "x"}}), "display_and_wait/hologram"),
        (json!({"type": "display_and_wait", "display_and_wait": {"type": "qr"}}), "display_and_wait/qr"),
        (json!({"type": "user_input", "user_input": {"fields": []}}), "user_input/empty"),
        (json!({"type": "user_input", "user_input": {"fields": [{"type": "select", "id": "country"}]}}), "user_input/country"),
    ] {
        assert!(matches!(with(extra.clone()), LoginStep::Unsupported { kind: k, .. } if k == kind), "{extra}");
    }
    assert!(matches!(login_step(&signal(), &json!({"type": "user_input"})), Err(BridgeError::Unreachable(_))), "a step without IDs");
    assert!(matches!(login_step(&signal(), &json!("nonsense")), Err(BridgeError::Unreachable(_))));
}

#[test]
fn a_complete_login_names_the_new_account() {
    let answer = json!({"login_id": "l", "type": "complete", "step_id": "s", "complete": {"user_login_id": "abc-123"}});
    assert_eq!(login_step(&signal(), &answer).unwrap(), LoginStep::Done { network: signal(), account: AccountId("abc-123".into()) });
}

#[test]
fn whoami_gives_flows_and_accounts_and_skips_what_is_malformed() {
    let whoami = json!({
        "network": {"displayname": "Signal", "network_id": "signal"},
        "login_flows": [{"name": "QR", "description": "Scan a QR code", "id": "qr"}, {"name": "no id"}, 7],
        "logins": [
            {"id": "u1", "name": "+45 12 34 56 78", "state": {"state_event": "CONNECTED"}, "profile": {}},
            {"id": "u2", "name": "", "state": {"state_event": "BAD_CREDENTIALS"}, "profile": {"username": "ana"}},
            {"name": "no id"},
            "nonsense"
        ]
    });
    let n = bridged_network(&signal(), &whoami);
    assert_eq!(n.flows.len(), 1);
    assert_eq!((n.flows[0].id.as_str(), n.flows[0].name.as_str()), ("qr", "QR"));
    assert_eq!(n.accounts.len(), 2);
    assert_eq!((n.accounts[0].id.0.as_str(), n.accounts[0].name.as_str(), &n.accounts[0].state), ("u1", "+45 12 34 56 78", &AccountState::Connected));
    assert_eq!((n.accounts[1].name.as_str(), &n.accounts[1].state), ("ana", &AccountState::LoggedOut));
    assert!(n.accounts.iter().all(|a| a.network == signal()));
    let empty = bridged_network(&signal(), &json!([]));
    assert!(empty.flows.is_empty() && empty.accounts.is_empty());
}

#[test]
fn bridge_states_map_to_account_states() {
    assert_eq!(account_state(&json!({"state_event": "TRANSIENT_DISCONNECT"})), AccountState::Connecting);
    assert_eq!(account_state(&json!({"state_event": "LOGGED_OUT"})), AccountState::LoggedOut);
    assert_eq!(account_state(&json!({"state_event": "UNKNOWN_ERROR", "message": "phone offline"})), AccountState::Error("phone offline".into()));
    assert_eq!(account_state(&json!({"state_event": "BRIDGE_UNREACHABLE"})), AccountState::Error("BRIDGE_UNREACHABLE".into()));
    assert_eq!(account_state(&Value::Null), AccountState::Connecting);
}

#[test]
fn errors_say_what_happened() {
    assert_eq!(error_for(401, Some(&json!({"errcode": "M_UNKNOWN_TOKEN", "error": "Invalid auth token"}))), BridgeError::NotAuthorized);
    assert_eq!(error_for(404, None), BridgeError::NotAvailable, "the proxy has no bridge for that network");
    assert_eq!(error_for(502, None), BridgeError::NotAvailable, "the bridge is not running");
    assert_eq!(error_for(404, Some(&json!({"errcode": "M_NOT_FOUND", "error": "Login not found"}))), BridgeError::LoginGone);
    assert_eq!(
        error_for(410, Some(&json!({"errcode": "FI.MAU.BRIDGE.LOGIN_TIMED_OUT", "error": "Login process timed out"}))),
        BridgeError::LoginGone
    );
    assert_eq!(
        error_for(403, Some(&json!({"errcode": "FI.MAU.BRIDGE.TOO_MANY_LOGINS", "error": "Maximum number of logins exceeded (1)"}))),
        BridgeError::Refused("Maximum number of logins exceeded (1)".into())
    );
    assert_eq!(error_for(500, Some(&json!({}))), BridgeError::Unreachable("the bridge answered 500".into()));
}

#[test]
fn ids_are_escaped_in_paths_and_queries() {
    assert_eq!(escape("@ana:example.org"), "%40ana%3Aexample.org");
    assert_eq!(escape("../app"), "..%2Fapp");
    assert_eq!(escape("fi.mau.signal.qr"), "fi.mau.signal.qr");
}

/// One request as the fake bridge saw it.
#[derive(Debug, Clone)]
struct Seen {
    line: String,
    auth: String,
    body: String,
}

struct FixedToken;

impl TokenSource for FixedToken {
    fn openid_token(&self) -> BoxFuture<'_, Result<Zeroizing<String>, BridgeError>> {
        Box::pin(async { Ok(Zeroizing::new("tok123".into())) })
    }
}

/// A fake bridge: answers each request with the next of `answers` (status, body), and records what it was sent.
async fn fake_bridge(answers: Vec<(u16, Value)>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        for (status, body) in answers {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head, mut rest) = loop {
                let n = socket.read(&mut chunk).await.unwrap();
                buf.extend_from_slice(&chunk[..n]);
                if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break (String::from_utf8_lossy(&buf[..end]).to_string(), buf[end + 4..].to_vec());
                }
            };
            let header = |name: &str| {
                head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_owned()))
            };
            let length: usize = header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
            while rest.len() < length {
                let n = socket.read(&mut chunk).await.unwrap();
                rest.extend_from_slice(&chunk[..n]);
            }
            log.lock().unwrap().push(Seen {
                line: head.lines().next().unwrap_or_default().to_owned(),
                auth: header("authorization").unwrap_or_default(),
                body: String::from_utf8_lossy(&rest).to_string(),
            });
            let body = if body.is_null() { String::new() } else { body.to_string() };
            let reply =
                format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    (base, seen)
}

#[tokio::test]
async fn a_login_goes_through_the_proxy_paths_with_an_openid_token() {
    let ask = json!({"login_id": "L 1", "type": "user_input", "step_id": "fi.mau.telegram.phone", "txn_id": "T1",
        "user_input": {"fields": [{"type": "phone_number", "id": "phone", "name": "Phone"}]}});
    let code = json!({"login_id": "L 1", "type": "user_input", "step_id": "fi.mau.telegram.code", "txn_id": "T2",
        "user_input": {"fields": [{"type": "2fa_code", "id": "code", "name": "Code"}]}});
    let (base, seen) = fake_bridge(vec![(200, ask), (200, code), (200, json!({})), (200, json!({}))]).await;
    let telegram = Network::from_id("telegram");
    let provider = MautrixProvider::with_networks(&format!("{base}/"), "@ana:example.org", Arc::new(FixedToken), vec![telegram.clone()]);

    let first = provider.start_login(&telegram, "phone").await.unwrap();
    let step = first.step().unwrap().clone();
    let mut answers = Answers::new();
    answers.set("phone", Zeroizing::new("+4512345678".into()));
    let second = provider.submit(&step, answers).await.unwrap();
    assert!(matches!(&second, LoginStep::Ask { fields, .. } if fields[0].kind == FieldKind::Code));
    provider.cancel(second.step().unwrap()).await.unwrap();
    provider.remove_account(&telegram, &AccountId("u/1".into())).await.unwrap();

    let seen = seen.lock().unwrap().clone();
    let lines: Vec<&str> = seen.iter().map(|s| s.line.as_str()).collect();
    let prefix = "/_pingcraft/bridges/telegram/_matrix/provision/v3";
    let user = "user_id=%40ana%3Aexample.org";
    assert_eq!(
        lines,
        [
            format!("POST {prefix}/login/start/phone?{user} HTTP/1.1"),
            format!("POST {prefix}/login/step/L%201/fi.mau.telegram.phone/user_input?{user}&txn_id=T1 HTTP/1.1"),
            format!("POST {prefix}/login/cancel/L%201?{user} HTTP/1.1"),
            format!("POST {prefix}/logout/u%2F1?{user} HTTP/1.1"),
        ]
    );
    assert!(seen.iter().all(|s| s.auth == "Bearer openid:tok123"), "every request proves who the user is with OpenID");
    assert_eq!(serde_json::from_str::<Value>(&seen[1].body).unwrap(), json!({"phone": "+4512345678"}));
}

#[tokio::test]
async fn a_missing_bridge_and_a_refused_token_are_told_apart() {
    let (base, _) = fake_bridge(vec![(404, Value::Null), (401, json!({"errcode": "M_UNKNOWN_TOKEN", "error": "Invalid auth token"}))]).await;
    let provider = MautrixProvider::with_networks(&base, "@ana:example.org", Arc::new(FixedToken), vec![signal()]);
    assert_eq!(provider.network(&signal()).await, Err(BridgeError::NotAvailable));
    assert_eq!(provider.network(&signal()).await, Err(BridgeError::NotAuthorized));
}

#[tokio::test]
async fn the_all_account_is_never_logged_out() {
    let provider = MautrixProvider::with_networks("http://127.0.0.1:9", "@ana:example.org", Arc::new(FixedToken), vec![signal()]);
    assert!(matches!(provider.remove_account(&signal(), &AccountId("all".into())).await, Err(BridgeError::Refused(_))));
}

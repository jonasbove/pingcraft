//! The ACCOUNTS tab: connecting a network after its risk screen, the login steps, and removing an account.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pingcraft_core::app::{Command, Event};
use pingcraft_core::bridge::{BridgeError, BridgedNetwork, Display, FieldKind, InputField, LoginFlow, LoginStep, StepRef};
use pingcraft_core::{Account, AccountId, AccountState, InboxChange, Network};

use super::{Fake, harness, signed_in, type_into};
use crate::PingCraftApp;

fn flow(id: &str, name: &str) -> LoginFlow {
    LoginFlow { id: id.into(), name: name.into(), description: format!("{name}, in the bridge's words") }
}

fn bridged(network: &str, flows: Vec<LoginFlow>, accounts: Vec<Account>) -> Event {
    let network = Network::from_id(network);
    Event::Network { network: network.clone(), found: Ok(BridgedNetwork { network, flows, accounts }) }
}

fn step_ref(network: &str, step: &str) -> StepRef {
    StepRef { network: Network::from_id(network), login: "login-1".into(), step: step.into(), attempt: "t".into() }
}

/// Signed in, on the ACCOUNTS tab, with the bridges' answers in.
fn on_accounts(fake: &Fake, accounts: Vec<Account>) -> Harness<'static, PingCraftApp> {
    let mut h = signed_in(fake);
    h.get_by_label("ACCOUNTS").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::LoadNetworks]), "opening the tab asks the bridges");
    fake.tell([
        bridged("telegram", vec![flow("phone", "Phone number"), flow("qr", "QR code")], vec![]),
        bridged("signal", vec![flow("qr", "QR")], accounts),
        bridged("whatsapp", vec![flow("qr", "QR"), flow("phone", "Pairing code")], vec![]),
    ]);
    h.run();
    h
}

/// Ticks the risk screen's box for a network and confirms it.
fn accept_risk(h: &mut Harness<'static, PingCraftApp>, network: &str) {
    h.get_by_label(&format!("Connect {network}")).click();
    h.run();
    h.get_by_label(&format!("I understand that {network} may restrict or ban this account, and I want to connect it anyway.")).click();
    h.run();
    h.get_by_label(&format!("→ CONNECT {}", network.to_uppercase())).click();
    h.run();
}

#[test]
fn the_header_has_tabs_and_settings_waits() {
    let fake = Fake::default();
    let h = signed_in(&fake);
    for tab in ["INBOX", "ACCOUNTS", "SETTINGS"] {
        h.get_by_label(tab);
    }
    assert!(h.get_by_label("SETTINGS").accesskit_node().is_disabled(), "no settings screen yet");

    let mut h = on_accounts(&fake, vec![]);
    h.get_by_label("Connect Telegram");
    assert!(h.get_by_label("Discord: not yet").accesskit_node().is_disabled(), "Discord waits for its bridge");
    assert!(h.query_all_by_label("NO ACCOUNTS YET").count() >= 2);
    h.get_by_label("0 accounts");
    h.get_by_label("INBOX").click();
    h.run();
    h.get_by_label_contains("Book club, on Telegram");
}

#[test]
fn connecting_shows_the_risk_first_and_waits_for_the_tick() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    h.get_by_label("Connect WhatsApp").click();
    h.run();
    h.get_by_label("Before you connect WhatsApp");
    h.get_by_label("RISK LEVEL · MEDIUM-HIGH");
    // The card behind the sheet says MEDIUM-HIGH too; the scale's cell is the selected one.
    use egui::accesskit::Toggled;
    let marked: Vec<Option<Toggled>> = h.get_all_by_label("MEDIUM-HIGH").map(|n| n.accesskit_node().toggled()).collect();
    assert!(marked.contains(&Some(Toggled::True)), "the scale marks WhatsApp's level: {marked:?}");
    let low: Vec<Option<Toggled>> = h.get_all_by_label("LOW").map(|n| n.accesskit_node().toggled()).collect();
    assert!(low.contains(&Some(Toggled::False)) && !low.contains(&Some(Toggled::True)), "{low:?}");
    h.get_by_label_contains("WhatsApp's terms don't allow unofficial apps");
    h.get_by_label_contains("the link drops");
    let connect = "→ CONNECT WHATSAPP";
    assert!(h.get_by_label(connect).accesskit_node().is_disabled(), "not before the risk is accepted");
    h.get_by_label(connect).click();
    h.run();
    assert!(fake.take().is_empty(), "nothing is sent before the tick");

    h.get_by_label("I understand that WhatsApp may restrict or ban this account, and I want to connect it anyway.").click();
    h.run();
    h.get_by_label(connect).click();
    h.run();
    // Two ways to log in: the user picks one.
    h.get_by_label("HOW DO YOU WANT TO LOG IN?");
    assert!(fake.take().is_empty());
    h.get_by_label("PAIRING CODE").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::StartLogin { network, flow }] if network.id() == "whatsapp" && flow == "phone"));
}

#[test]
fn cancelling_the_risk_screen_sends_nothing() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    h.get_by_label("Connect Telegram").click();
    h.run();
    h.get_by_label("Before you connect Telegram");
    h.get_by_label("RISK LEVEL · LOW");
    h.get_by_label("CANCEL").click();
    h.run();
    assert!(h.query_by_label("Before you connect Telegram").is_none());
    assert!(fake.take().is_empty());
}

#[test]
fn a_qr_code_is_shown_until_cancelled() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    accept_risk(&mut h, "Signal");
    // One way to log in: it starts at once.
    assert!(matches!(&fake.take()[..], [Command::StartLogin { network, flow }] if network.id() == "signal" && flow == "qr"));
    h.get_by_label("Waiting for the bridge on your server…");

    let qr = LoginStep::Show {
        step: step_ref("signal", "fi.mau.signal.qr"),
        instructions: "Scan the QR code on your other device".into(),
        display: Display::Qr("sgnl://linkdevice?uuid=abc".into()),
        can_cancel: false,
    };
    fake.tell([Event::Login(qr)]);
    h.run();
    let code = h.get_by_label("QR code to scan with Signal");
    assert_eq!(code.accesskit_node().role(), egui::accesskit::Role::Image);
    h.get_by_label("Scan the QR code on your other device");
    h.get_by_label("CANCEL").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::CancelLogin(step)] if *step == step_ref("signal", "fi.mau.signal.qr")));
    assert!(h.query_by_label("QR code to scan with Signal").is_none());
}

#[test]
fn a_question_is_answered_and_done_closes_the_sheet() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    accept_risk(&mut h, "Telegram");
    h.get_by_label("PHONE NUMBER").click();
    h.run();
    fake.take();
    let field = InputField {
        id: "phone".into(),
        kind: FieldKind::PhoneNumber,
        name: "Phone number".into(),
        description: "With the country code".into(),
        default: None,
        optional: false,
    };
    fake.tell([Event::Login(LoginStep::Ask {
        step: step_ref("telegram", "phone"),
        instructions: String::new(),
        fields: vec![field],
        can_cancel: true,
    })]);
    h.run();
    assert!(h.get_by_label("CONTINUE").accesskit_node().is_disabled());
    type_into(&mut h, "PHONE NUMBER", "+4512345678");
    h.get_by_label("CONTINUE").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::AnswerLogin { step, answers }] if *step == step_ref("telegram", "phone") && answers.get("phone") == Some("+4512345678")),
        "{sent:?}"
    );
    fake.tell([Event::Login(LoginStep::Done { network: Network::from_id("telegram"), account: AccountId("new".into()) })]);
    h.run();
    assert!(h.query_by_label("CONTINUE").is_none(), "the sheet closed");
    h.get_by_label_contains("Telegram is connected.");
}

#[test]
fn a_failed_login_says_why() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    accept_risk(&mut h, "Signal");
    fake.tell([Event::LoginFailed { network: Network::from_id("signal"), error: BridgeError::NotAuthorized }]);
    h.run();
    h.get_by_label_contains("could not confirm who you are");
    h.get_by_label("CLOSE").click();
    h.run();
    assert!(h.query_by_label_contains("could not confirm who you are").is_none());
}

#[test]
fn removing_an_account_asks_first() {
    let fake = Fake::default();
    let account =
        Account { id: AccountId("u1".into()), network: Network::from_id("signal"), name: "+45 12 34 56 78".into(), state: AccountState::Connected };
    let mut h = on_accounts(&fake, vec![account]);
    h.get_by_label("YOUR ACCOUNTS · 1 ACCOUNT");
    h.get_by_label("+45 12 34 56 78");
    h.get_by_label("Remove +45 12 34 56 78").click();
    h.run();
    h.get_by_label("Remove this account?");
    assert!(fake.take().is_empty(), "nothing is removed before it is confirmed");
    h.get_by_label("CANCEL").click();
    h.run();
    assert!(h.query_by_label("Remove this account?").is_none());
    assert!(fake.take().is_empty());

    h.get_by_label("Remove +45 12 34 56 78").click();
    h.run();
    h.get_by_label("REMOVE ACCOUNT").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::RemoveAccount { network, account }] if network.id() == "signal" && account.0 == "u1"));
    fake.tell([Event::AccountRemoved { network: Network::from_id("signal"), account: AccountId("u1".into()) }]);
    h.run();
    h.get_by_label("The account was removed.");
    assert!(h.query_by_label("Remove +45 12 34 56 78").is_none());
}

#[test]
fn an_empty_inbox_leads_to_connecting_a_network() {
    let fake = Fake::default();
    let mut h = harness(&fake);
    fake.tell([Event::SignedIn { user: "@ana:example.org".into() }, Event::Inbox(InboxChange::Reset(vec![]))]);
    h.run();
    h.get_by_label("Your chats, one inbox.");
    h.get_by_label("CONNECT A NETWORK").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::LoadNetworks]));
    h.get_by_label("Connect Telegram");
}

#[test]
fn the_cards_and_the_risk_sheet_fit_the_window() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    let mut left = 0.0;
    for label in ["Connect Telegram", "Connect Signal", "Connect WhatsApp", "Discord: not yet"] {
        let r = h.get_by_label(label).accesskit_node().raw_bounds().unwrap();
        assert!(r.x0 >= left && r.x1 <= 1200.0, "{label} at {r:?}: the cards sit side by side inside the window");
        left = r.x1;
    }
    h.get_by_label("Connect WhatsApp").click();
    h.run();
    let go = h.get_by_label("→ CONNECT WHATSAPP").accesskit_node().raw_bounds().unwrap();
    let title = h.get_by_label("Before you connect WhatsApp").accesskit_node().raw_bounds().unwrap();
    assert!(title.y0 >= 0.0 && go.y1 <= 760.0, "the whole risk sheet is in the window: {title:?} to {go:?}");
}

#[test]
fn a_password_is_sent_as_typed_and_never_shown_by_debug() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    accept_risk(&mut h, "Telegram");
    h.get_by_label("PHONE NUMBER").click();
    h.run();
    fake.take();
    let field = |id: &str, kind: FieldKind, name: &str| InputField {
        id: id.into(),
        kind,
        name: name.into(),
        description: String::new(),
        default: None,
        optional: false,
    };
    let fields = vec![field("code", FieldKind::Code, "Code"), field("password", FieldKind::Password, "Two-step password")];
    fake.tell([Event::Login(LoginStep::Ask { step: step_ref("telegram", "2fa"), instructions: String::new(), fields, can_cancel: true })]);
    h.run();
    type_into(&mut h, "CODE", " 12345 ");
    type_into(&mut h, "TWO-STEP PASSWORD", " spaces count ");
    let shown = format!("{:?}", h.state().state.accounts);
    assert!(!shown.contains("12345") && !shown.contains("spaces count"), "{shown}");
    h.get_by_label("CONTINUE").click();
    h.run();
    let sent = fake.take();
    let [Command::AnswerLogin { answers, .. }] = &sent[..] else { panic!("{sent:?}") };
    assert_eq!(answers.get("code"), Some("12345"), "spaces around a code are dropped");
    assert_eq!(answers.get("password"), Some(" spaces count "), "a password is sent exactly as typed");
}

#[test]
fn a_connected_network_says_so_instead_of_offering_to_connect() {
    let fake = Fake::default();
    let account =
        Account { id: AccountId("u1".into()), network: Network::from_id("signal"), name: "+45 12 34 56 78".into(), state: AccountState::Connected };
    let h = on_accounts(&fake, vec![account]);
    h.get_by_label("Signal: connected");
    assert!(h.query_by_label("Connect Signal").is_none(), "no CONNECT button for a network that is connected");
    h.get_by_label("Connect Telegram");
}

#[test]
fn a_logged_out_account_offers_to_reconnect() {
    let fake = Fake::default();
    let account =
        Account { id: AccountId("u1".into()), network: Network::from_id("signal"), name: "+45 12 34 56 78".into(), state: AccountState::LoggedOut };
    let h = on_accounts(&fake, vec![account]);
    assert!(h.query_by_label("Signal: connected").is_none());
    h.get_by_label("Reconnect Signal");
}

#[test]
fn every_card_has_its_account_line_and_button_at_the_same_height() {
    let fake = Fake::default();
    let account =
        Account { id: AccountId("u1".into()), network: Network::from_id("signal"), name: "+45 12 34 56 78".into(), state: AccountState::Connected };
    let mut h = on_accounts(&fake, vec![account]);
    // The cards line up from the tallest description measured on the frame before.
    h.run();
    let top = |label: &str| h.get_by_label(label).accesskit_node().raw_bounds().unwrap().y0;
    let telegram = top("Connect Telegram");
    for other in ["Signal: connected", "Connect WhatsApp", "Discord: not yet"] {
        assert!((top(other) - telegram).abs() < 1.0, "{other} is at {}, Telegram's button at {telegram}", top(other));
    }
}

#[test]
fn an_account_row_leaves_room_between_the_tag_the_name_and_the_state() {
    let fake = Fake::default();
    let account =
        Account { id: AccountId("u1".into()), network: Network::from_id("signal"), name: "+45 12 34 56 78".into(), state: AccountState::Connected };
    let h = on_accounts(&fake, vec![account]);
    let name = h.get_by_label("+45 12 34 56 78").accesskit_node().raw_bounds().unwrap();
    let tag = h
        .get_all_by_label("SIGNAL")
        .map(|n| n.accesskit_node().raw_bounds().unwrap())
        .find(|r| (r.y0 + r.y1) / 2.0 > name.y0 && (r.y0 + r.y1) / 2.0 < name.y1)
        .unwrap();
    let state = h.get_all_by_label("Connected").map(|n| n.accesskit_node().raw_bounds().unwrap()).find(|r| r.x0 > name.x1).unwrap();
    assert!(name.x0 - tag.x1 >= 16.0, "tag to name: {}", name.x0 - tag.x1);
    assert!(state.x0 - name.x1 >= 16.0, "name to state: {}", state.x0 - name.x1);
}

#[test]
fn refresh_is_on_every_tab() {
    let fake = Fake::default();
    let mut h = on_accounts(&fake, vec![]);
    h.get_by_label("REFRESH").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Refresh]));
    h.get_by_label("INBOX").click();
    h.run();
    h.get_by_label("REFRESH").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::Refresh]));
}

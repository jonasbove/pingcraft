//! The SETTINGS tab through its AccessKit tree: opening it (its tab, Ctrl+, and the app menu), and each page's
//! controls and the commands they send.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pingcraft_core::app::{Command, Event};
use pingcraft_core::devices::{Device, DeviceId, SignOutProblem};
use pingcraft_core::encryption::{RecoveryKey, RecoveryStatus};
use pingcraft_core::{Alerts, Network, Timestamp};

use super::{Fake, click_in_footer, signed_in, type_into};
use crate::{Appearance, PingCraftApp, Tab};

fn device(id: &str, name: &str, verified: bool, this: bool) -> Device {
    Device { id: DeviceId(id.into()), name: name.into(), last_seen: Some(Timestamp(1_795_000_000_000)), verified, this }
}

/// Signed in, on the SETTINGS tab, showing `page` (by its title in the page list).
fn on_page(fake: &Fake, page: &str) -> Harness<'static, PingCraftApp> {
    let mut h = signed_in(fake);
    h.get_by_role_and_label(Role::Button, "Settings").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::LoadDevices]), "opening Settings asks for the devices");
    fake.tell([Event::Devices(vec![device("THIS", "Ana's laptop", true, true), device("PHONE", "Pixel", false, false)])]);
    h.run();
    pick_page(&mut h, page);
    h
}

/// Shows `page` by clicking it in the page list.
fn pick_page(h: &mut Harness<'static, PingCraftApp>, page: &str) {
    let list = h.get_by_label("Pages");
    list.get_by_label(page).click();
    h.run();
}

#[test]
fn settings_opens_from_its_tab_and_from_ctrl_comma_with_its_four_pages() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Comma);
    h.run();
    h.run();
    assert_eq!(h.state().state.tab, Tab::Settings, "Ctrl+, opens Settings");
    assert!(fake.take().iter().any(|c| matches!(c, Command::LoadDevices)));
    let list = h.get_by_label("Pages");
    for page in ["Appearance", "Notifications", "Emoji", "Security"] {
        list.get_by_label(page);
    }
    h.get_by_role_and_label(Role::Button, "Inbox").click();
    h.run();
    assert_eq!(h.state().state.tab, Tab::Inbox);
}

#[test]
fn the_app_menu_opens_settings() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label_contains("Preferences").click();
    h.run();
    h.run();
    assert_eq!(h.state().state.tab, Tab::Settings, "Edit, Preferences… opens Settings");
}

#[test]
fn appearance_is_light_dark_or_the_systems() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Appearance");
    assert_eq!(h.state().state.appearance(), Appearance::System, "follows the system unless chosen");
    h.get_by_label("Dark").click();
    h.run();
    h.run();
    assert_eq!(h.state().state.appearance(), Appearance::Dark);
    assert!(crate::Tokens::get(&h.ctx).dark, "the theme is dark");
    h.get_by_label("Light").click();
    h.run();
    h.run();
    assert!(!crate::Tokens::get(&h.ctx).dark, "and light again");
}

/// A theme only PingCraft has is PingCraft's own choice; a craft-libs theme goes to the Craft suite's choice unless
/// "Just for PingCraft" is on. The window takes the theme's colours.
#[test]
fn themes_are_picked_for_each_mode_and_kept_here_or_with_the_suite() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Appearance");
    h.get_by_label("Dark").click();
    h.run();
    h.run();
    let before = crate::Tokens::get(&h.ctx).bg;
    let pick = |h: &mut Harness<'static, PingCraftApp>, row: &str, theme: &str| {
        h.get_by_role_and_label(Role::RadioGroup, row).get_by_label_contains(theme).click();
        h.run();
        h.run();
        h.run();
    };
    pick(&mut h, "Dark theme", "Tokyo Night");
    let s = &h.state().state;
    assert_eq!(s.selection().theme.dark.as_str(), "pingcraft.tokyo-night");
    assert!(s.prefs.just_here(), "a theme only PingCraft has is PingCraft's own");
    assert_eq!(s.suite.theme_dark, None, "the suite is not told of it");
    assert_ne!(crate::Tokens::get(&h.ctx).bg, before, "the window takes its colours");

    h.get_by_role_and_label(Role::CheckBox, "Just for PingCraft").click();
    h.run();
    h.run();
    assert!(!h.state().state.prefs.just_here(), "back to the suite's");
    pick(&mut h, "Dark theme", "Graphite");
    let s = &h.state().state;
    assert_eq!(s.suite.theme_dark.as_ref().map(|t| t.as_str()), Some("craft.graphite"), "a craft-libs theme goes to the suite");
    assert!(!s.prefs.just_here());
}

/// The saved appearance applies from the very first frame: a window set to dark never shows a light frame first.
#[test]
fn a_saved_dark_appearance_shows_from_the_first_frame() {
    let ctx = super::context();
    let mut app = PingCraftApp::new(Box::new(Fake::default()));
    app.set_prefs(crate::Prefs { look: craft_theme::AppChoice::new(None, Some(craft_theme::ModeChoice::Dark), None), previews: true });
    app.prime(&ctx);
    // One pass, the window's first: the theme it resolves is dark.
    let mut dark = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| dark = Some(crate::Tokens::get(ui.ctx()).dark));
    output.textures_delta.clear();
    assert_eq!(dark, Some(true), "dark from the first frame");
    let mut h = egui_kittest::Harness::builder().with_context(ctx).with_size(egui::vec2(1200.0, 760.0)).build_ui_state(super::frame, app);
    h.run();
    assert!(crate::Tokens::get(&h.ctx).dark, "and after");
}

#[test]
fn notifications_set_previews_and_each_networks_sound_and_colour() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Notifications");
    h.get_by_role_and_label(Role::CheckBox, "Show message text in notifications").click();
    h.run();
    assert!(!h.state().state.prefs.previews, "previews off");
    assert!(matches!(&fake.take()[..], [Command::SavePreviews(false)]), "kept with the account");

    // Each network's rows are in a group named by it.
    h.get_by_role_and_label(Role::Group, "Telegram").get_by_role_and_label(Role::CheckBox, "Notification sound").click();
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::NetworkAlerts { network, alerts: Alerts { sound: Some(false), accent: None } }] if *network == Network::from_id("telegram")),
        "{sent:?}"
    );
    // Any colour, typed as hex; a second change builds on the first.
    h.get_by_role_and_label(Role::Group, "Telegram").get_by_role_and_label(Role::TextInput, "Accent colour").focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run();
    h.get_by_role_and_label(Role::Group, "Telegram").get_by_role_and_label(Role::TextInput, "Accent colour").type_text("123456");
    h.key_press(egui::Key::Enter);
    h.run();
    let sent = fake.take();
    assert!(
        matches!(&sent[..], [Command::NetworkAlerts { network, alerts: Alerts { sound: Some(false), accent: Some(a) } }] if *network == Network::from_id("telegram") && a.0 == [0x12, 0x34, 0x56]),
        "{sent:?}"
    );
}

/// Moving through the Notifications page by keyboard changes nothing: a field committing the value it shows is no edit.
#[test]
fn tabbing_through_notifications_sends_nothing() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Notifications");
    h.get_by_role_and_label(Role::Group, "Telegram").get_by_role_and_label(Role::TextInput, "Accent colour").focus();
    h.run();
    for _ in 0..12 {
        h.key_press(egui::Key::Tab);
        h.run();
    }
    assert!(fake.take().is_empty(), "nothing was changed");
}

/// A failure while a sheet waits leaves it usable again, and a device list comes again when a verification ends.
#[test]
fn a_failure_frees_the_waiting_sheet_and_a_verification_reloads_the_devices() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    h.get_by_role_and_label(Role::Button, "Create a new recovery key…").click();
    h.run();
    h.run();
    click_in_footer(&mut h, "Create a new key");
    h.run();
    fake.take();
    fake.tell([Event::Problem("the server said no".into())]);
    h.run();
    assert!(!h.get_by_role_and_label(Role::Button, "Create a new key").accesskit_node().is_disabled(), "it can be tried again");
    fake.tell([Event::Verification(None)]);
    h.run();
    assert!(fake.take().iter().any(|c| matches!(c, Command::LoadDevices)));
}

/// The account's previews setting wins; an account that keeps none is given this computer's "off" once.
#[test]
fn previews_follow_the_account_and_a_local_off_moves_up() {
    let fake = Fake::default();
    let mut h = super::signed_in(&fake);
    fake.take();
    fake.tell([Event::Previews(Some(false))]);
    h.run();
    assert!(!h.state().state.prefs.previews, "the account's");
    fake.tell([Event::Previews(None)]);
    h.run();
    assert!(matches!(&fake.take()[..], [Command::SavePreviews(false)]), "this computer's off moves up");
    h.state_mut().state.prefs.previews = true;
    fake.tell([Event::Previews(None)]);
    h.run();
    assert!(fake.take().is_empty(), "the default is not sent");
}

#[test]
fn a_colour_swatch_opens_the_colour_picker() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Notifications");
    h.get_by_role_and_label(Role::Group, "Signal").get_by_role_and_label(Role::ColorWell, "Accent colour").click();
    h.run();
    h.get_by_role(Role::Dialog);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_role(Role::Dialog).is_none(), "Escape closes it");
    assert!(fake.take().is_empty(), "a cancelled pick changes nothing");
}

#[test]
fn the_emoji_page_sets_the_default_skin_tone() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Emoji");
    h.get_by_role_and_label(Role::RadioButton, "Medium-dark").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::SaveSkinTone(t)] if t == "medium-dark"), "{sent:?}");
}

#[test]
fn the_passphrase_changes_only_where_there_is_one() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    fake.tell([Event::Passphrase(false)]);
    h.run();
    h.get_by_label_contains("keychain");
    assert!(h.query_by_role_and_label(Role::Button, "Change passphrase…").is_none());

    fake.tell([Event::Passphrase(true)]);
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Change passphrase…").click();
    h.run();
    h.run();
    type_into(&mut h, "Current passphrase", "old words");
    type_into(&mut h, "New passphrase", "new words");
    type_into(&mut h, "New passphrase again", "new word");
    h.get_by_label("The new passphrases are not the same.");
    assert!(h.get_by_role_and_label(Role::Button, "Change passphrase").accesskit_node().is_disabled());
    type_into(&mut h, "New passphrase again", "s");
    h.get_by_role_and_label(Role::Button, "Change passphrase").click();
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::ChangePassphrase { current, new }] if **current == "old words" && **new == "new words"), "{sent:?}");
    fake.tell([Event::PassphraseChanged(false)]);
    h.run();
    h.get_by_label("That is not the current passphrase.");
    fake.tell([Event::PassphraseChanged(true)]);
    h.run();
    h.run();
    assert!(h.state().state.settings.passphrase.is_none(), "the sheet closes");
    h.get_by_label_contains("The passphrase was changed.");
}

#[test]
fn this_device_is_renamed_and_signed_out_with_a_warning_without_a_recovery_key() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    let name = h.get_by_role_and_label(Role::TextInput, "This device's name");
    assert_eq!(name.accesskit_node().value().as_deref(), Some("Ana's laptop"));
    type_into(&mut h, "This device's name", " at home");
    h.key_press(egui::Key::Enter);
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::RenameDevice(n)] if n == "Ana's laptop at home"), "{sent:?}");

    h.get_by_role_and_label(Role::Button, "Sign out…").click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Dialog, "Sign out of this device?");
    assert!(h.query_by_label_contains("no recovery key").is_none(), "recovery is set up");
    click_in_footer(&mut h, "Cancel");
    h.run();
    fake.tell([Event::Recovery(RecoveryStatus::Off)]);
    h.run();
    h.get_by_role_and_label(Role::Button, "Sign out…").click();
    h.run();
    h.run();
    h.get_by_label_contains("no recovery key set up");
    click_in_footer(&mut h, "Sign out");
    h.run();
    assert!(matches!(&fake.take()[..], [Command::SignOut]));
}

#[test]
fn another_device_is_verified_or_signed_out_with_the_account_password() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    h.get_by_label("Pixel");
    h.get_by_label_contains("Not verified");
    h.get_by_role_and_label(Role::Button, "Verify Pixel").click();
    h.run();
    assert!(matches!(&fake.take()[..], [Command::VerifyDevice(DeviceId(id))] if id == "PHONE"));

    h.get_by_role_and_label(Role::Button, "Sign out Pixel…").click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Dialog, "Sign this device out?");
    type_into(&mut h, "Account password", "hunter2");
    click_in_footer(&mut h, "Sign out");
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::SignOutDevice { device: DeviceId(id), password }] if id == "PHONE" && **password == "hunter2"), "{sent:?}");
    assert!(h.state().state.settings.device_sign_out.as_ref().is_some_and(|d| d.password.is_empty()), "the password leaves the form once sent");
    fake.tell([Event::SignOutRefused { device: DeviceId("PHONE".into()), problem: SignOutProblem::WrongPassword }]);
    h.run();
    h.get_by_label("That password was not accepted.");
    fake.tell([Event::Devices(vec![device("THIS", "Ana's laptop", true, true)])]);
    h.run();
    h.run();
    assert!(h.state().state.settings.device_sign_out.is_none());
    h.get_by_label("You are not signed in anywhere else.");
}

#[test]
fn a_new_recovery_key_is_asked_for_and_shown_once() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    h.get_by_label_contains("Set up. This device can read");
    h.get_by_role_and_label(Role::Button, "Create a new recovery key…").click();
    h.run();
    h.run();
    h.get_by_label_contains("Your current recovery key stops working");
    click_in_footer(&mut h, "Create a new key");
    h.run();
    assert!(matches!(&fake.take()[..], [Command::NewRecoveryKey { password: None }]), "rotating a key this device holds needs no password");
    fake.tell([Event::NewRecoveryKey(RecoveryKey::new("EsTc 8Jq2 aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj"))]);
    h.run();
    h.run();
    h.get_by_label_contains("New recovery key");
    // Without a key this device can open, none is offered: entering it is.
    fake.tell([Event::Recovery(RecoveryStatus::NeedsKey)]);
    h.state_mut().state.recovery_forms.new_key = None;
    h.run();
    h.run();
    assert!(h.query_by_role_and_label(Role::Button, "Create a new recovery key…").is_none());
    h.get_by_role_and_label(Role::Button, "Enter recovery key…");
}

#[test]
fn a_lost_recovery_key_is_reset_after_a_warning_and_the_account_password() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    fake.tell([Event::Recovery(RecoveryStatus::NeedsKey)]);
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Reset: make a new recovery key…").click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Dialog, "Reset your recovery key?");
    h.get_by_label_contains("can no longer be read on a new device");
    assert!(h.get_by_role_and_label(Role::Button, "Reset and make a new key").accesskit_node().is_disabled(), "not without the password");
    type_into(&mut h, "Account password", "hunter2");
    click_in_footer(&mut h, "Reset and make a new key");
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::ResetRecovery { password }] if **password == "hunter2"), "{sent:?}");
    fake.tell([Event::RecoveryKeyRefused(pingcraft_core::encryption::RecoveryRefusal::WrongPassword)]);
    h.run();
    h.get_by_label("That password was not accepted.");
}

#[test]
fn a_first_recovery_key_takes_the_account_password_when_given() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    fake.tell([Event::Recovery(RecoveryStatus::Off)]);
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Create a new recovery key…").click();
    h.run();
    h.run();
    type_into(&mut h, "Account password", "hunter2");
    click_in_footer(&mut h, "Create a new key");
    h.run();
    let sent = fake.take();
    assert!(matches!(&sent[..], [Command::NewRecoveryKey { password: Some(p) }] if **p == "hunter2"), "{sent:?}");
}

#[test]
fn tab_reaches_the_security_pages_buttons_and_enter_runs_them() {
    let fake = Fake::default();
    let mut h = on_page(&fake, "Security");
    fake.tell([Event::Passphrase(true)]);
    h.run();
    let target = "Change passphrase…";
    let mut reached = false;
    for _ in 0..40 {
        h.key_press(egui::Key::Tab);
        h.run();
        if h.get_by_role_and_label(Role::Button, target).accesskit_node().is_focused() {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches {target}");
    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert!(h.state().state.settings.passphrase.is_some(), "Enter runs it");
}

/// A theme file of each kind PingCraft reads, imported from the Appearance page: kept in the suite library, shown
/// among the chips, chosen for its mode, and the window takes its colours.
#[test]
fn themes_are_imported_from_a_file_and_chosen() {
    let dir = std::env::temp_dir().join(format!("pingcraft-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let files = [
        ("harbour.yaml", "scheme: \"Harbour Night\"\nbase00: \"1b1d24\"\nbase01: \"23262f\"\nbase02: \"2e323d\"\nbase03: \"4b5263\"\nbase04: \"8b93a6\"\nbase05: \"d4d8e2\"\nbase08: \"e06c75\"\nbase0A: \"e5c07b\"\nbase0B: \"98c379\"\nbase0C: \"56b6c2\"\nbase0D: \"61afef\"\n".to_owned(), "imported.harbour-night"),
        ("paper.json", r##"{ // comments as VS Code allows
            "name": "Paper Morning", "type": "light",
            "colors": { "editor.background": "#fbf8f1", "editor.foreground": "#2b2a27", "button.background": "#2f6fbf" } }"##.to_owned(), "imported.paper-morning"),
        ("moss.json", include_str!("../../themes/gruvbox.json").replace("craft.gruvbox", "mine.moss").replace("\"Gruvbox\"", "\"Moss\""), "mine.moss"),
    ];
    let fake = Fake::default();
    let mut h = on_page(&fake, "Appearance");
    for (file, text, id) in files {
        let path = dir.join(file);
        std::fs::write(&path, text).unwrap();
        // The mode the theme is for, so the window shows the slot it goes in.
        h.get_by_label(if id == "imported.paper-morning" { "Light" } else { "Dark" }).click();
        h.run();
        h.run();
        let before = crate::Tokens::get(&h.ctx).bg;
        type_into(&mut h, "Theme file", &path.to_string_lossy());
        h.get_by_role_and_label(Role::Button, "Import theme").click();
        h.run();
        h.run();
        h.run();
        h.get_by_label_contains("was imported and chosen");
        let s = &h.state().state;
        let shown = s.selection();
        assert!(shown.theme.dark.as_str() == id || shown.theme.light.as_str() == id, "{file}: {shown:?}");
        assert_ne!(crate::Tokens::get(&h.ctx).bg, before, "{file}: the window takes its colours");
        h.get_by_label_contains("Close").click();
        h.run();
    }
    // A file that is no theme says so and changes nothing.
    let bad = dir.join("notes.txt");
    std::fs::write(&bad, "hello").unwrap();
    type_into(&mut h, "Theme file", &bad.to_string_lossy());
    h.get_by_role_and_label(Role::Button, "Import theme").click();
    h.run();
    h.get_by_label_contains("was not imported");
    let _ = std::fs::remove_dir_all(&dir);
}

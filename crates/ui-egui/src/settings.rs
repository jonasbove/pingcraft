//! The SETTINGS tab: craft-libs' preferences pattern (`craft_ui::patterns::preferences::PreferencesPage`, laid out as
//! it lays pages out by default: a search field, the page list and the shown page), with each edit applied at once.
//!
//! - **Appearance:** light, dark, or as the system is (craft-theme's mode). Colour themes come later (PLAN.md, M6
//!   ticket 7), and the page says so.
//! - **Notifications:** message text in notifications, kept on this computer; each network's sound and accent
//!   colour (craft-color-ui's colour picker), kept with the account.
//! - **Emoji:** the default skin tone, kept with the account.
//! - **Security:** this device's name, the unlock passphrase, signing this device out, the recovery key, and the other
//!   devices, each of which can be verified or signed out.
//!
//! What a page's buttons do are PingCraft's registered commands ([`crate::commands::AppCommand`]); the sheets they
//! open are drawn by [`sheets`]. Values the account keeps change through the backend's commands.

use std::collections::BTreeMap;

use egui::Ui;
use pingcraft_core::app::{Command, Event};
use pingcraft_core::devices::{Device, DeviceId, SignOutProblem, clean_name};
use pingcraft_core::encryption::{RecoveryRefusal, RecoveryStatus};
use pingcraft_core::{Accent, Alerts, Network};
use zeroize::Zeroizing;

use craft_theme::{Rgba, Srgba, ThemeDef, Variants};
use craft_theme_store::ui::ThemeChip;
use craft_ui::buttons::Button;
use craft_ui::choice::RadioGroup;
use craft_ui::contract::{Bind, Entry, Props as _, Show as _, ShowContainer as _};
use craft_ui::display::{Shade, Text};
use craft_ui::layout_components::{Row, Stack};
use craft_ui::patterns::preferences::{GroupSpec, PageSpec, PrefValue, PreferencesModel, PreferencesPage, PreferencesState, RowKind, RowSpec};
use craft_ui::runtime::requests::ColorRequest;
use craft_ui::style::{TypeRole, Variant};

use crate::commands::{AppCommand, SettingAction};
use crate::i18n::fmt;
use crate::picker::SkinTone;
use crate::state::{Appearance, LookChange, State};
use crate::theme::Tokens;
use crate::tl;
use crate::view::Out;
use crate::widgets::{Kind, button, field, network_name, note, problem};

/// The networks whose sound and colour Settings sets, in the order it lists them: the bridged ones, then Matrix.
const NETWORKS: [&str; 5] = ["telegram", "signal", "whatsapp", "discord", "matrix"];

/// The Settings tab's state, apart from the values it shows (those are the app's and the account's).
#[derive(Default)]
pub struct SettingsState {
    /// The preferences page's own state: the page shown and the search.
    pub page: PreferencesState,
    /// The user's devices, once the backend has said; this one first.
    pub devices: Option<Vec<Device>>,
    /// The sheet that changes the unlock passphrase.
    pub passphrase: Option<PassphraseForm>,
    /// The sheet that asks before making a new recovery key, or resetting a lost one.
    pub new_key: Option<KeyForm>,
    /// The sheet that asks before signing this device out.
    pub sign_out: bool,
    /// The sheet that signs another device out.
    pub device_sign_out: Option<DeviceSignOut>,
    /// A line about what just happened (the passphrase changed, say).
    pub notice: Option<String>,
    /// The name this device was last given, until the list says it has it (the field commits more than once).
    pub renamed: Option<String>,
    /// The path typed into the theme import field.
    pub import_path: String,
}

/// The passphrase sheet's fields, wiped when dropped.
#[derive(Default)]
pub struct PassphraseForm {
    pub current: Zeroizing<String>,
    pub new: Zeroizing<String>,
    pub again: Zeroizing<String>,
    /// Sent, waiting for the answer.
    pub sent: bool,
    /// The current passphrase typed was wrong.
    pub wrong: bool,
}

/// The new recovery key sheet: whether it resets a lost key, the account password (wiped when dropped), and how it
/// went.
#[derive(Default)]
pub struct KeyForm {
    pub reset: bool,
    pub password: Zeroizing<String>,
    pub sent: bool,
    pub refused: Option<RecoveryRefusal>,
}

/// Signing another device out: which one, and the account password the server asks for (wiped when dropped).
pub struct DeviceSignOut {
    pub device: Device,
    pub password: Zeroizing<String>,
    pub sent: bool,
    pub problem: Option<SignOutProblem>,
}

impl SettingsState {
    /// Takes in what the backend said that Settings shows.
    pub fn apply(&mut self, event: &Event) {
        match event {
            Event::Devices(devices) => {
                // A device that is gone is no longer being signed out.
                if let Some(d) = &self.device_sign_out
                    && !devices.iter().any(|o| o.id == d.device.id)
                {
                    self.notice = Some(fmt(tl!("{device} was signed out."), &[("device", &device_name(&d.device))]));
                    self.device_sign_out = None;
                }
                self.devices = Some(devices.clone());
                self.renamed = None;
            }
            Event::SignOutRefused { device, problem } => {
                if let Some(d) = self.device_sign_out.as_mut().filter(|d| d.device.id == *device) {
                    d.sent = false;
                    d.password = Zeroizing::default();
                    d.problem = Some(problem.clone());
                }
            }
            Event::PassphraseChanged(changed) => {
                if *changed {
                    self.passphrase = None;
                    self.notice = Some(tl!("The passphrase was changed. Use the new one next time PingCraft starts.").to_owned());
                } else if let Some(form) = self.passphrase.as_mut() {
                    form.sent = false;
                    form.wrong = true;
                    form.current = Zeroizing::default();
                }
            }
            Event::NewRecoveryKey(_) => self.new_key = None,
            Event::RecoveryKeyRefused(why) => {
                if let Some(form) = self.new_key.as_mut() {
                    form.sent = false;
                    form.password = Zeroizing::default();
                    form.refused = Some(*why);
                }
            }
            // Whatever was waiting failed (the problem bar says why): its sheet can be used again.
            Event::Problem(_) => {
                if let Some(form) = self.new_key.as_mut() {
                    form.sent = false;
                }
                if let Some(form) = self.passphrase.as_mut() {
                    form.sent = false;
                }
                if let Some(d) = self.device_sign_out.as_mut() {
                    d.sent = false;
                }
                self.renamed = None;
            }
            _ => {}
        }
    }

    /// This device, once the list has come.
    fn this_device(&self) -> Option<&Device> {
        self.devices.as_ref().and_then(|d| d.iter().find(|d| d.this))
    }
}

/// A device's name as listed: its name, or its ID when it has none.
fn device_name(device: &Device) -> String {
    if device.name.trim().is_empty() { device.id.0.clone() } else { device.name.clone() }
}

/// The key of a network's row: `notifications.<network>.<what>`.
fn network_key(id: &str, what: &str) -> String {
    format!("notifications.{id}.{what}")
}

/// The network a row key names, and what it sets.
fn parse_network_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("notifications.")?;
    let (id, what) = rest.split_once('.')?;
    NETWORKS.contains(&id).then_some((id, what))
}

/// An accent colour as a preferences colour.
fn rgba([r, g, b]: [u8; 3]) -> Rgba {
    Rgba([r, g, b, 0xff])
}

/// The words of the page that are built once per frame, so the rows can borrow them.
struct Words {
    /// For each network: its row keys and its name.
    networks: Vec<(String, String, String, Network)>,
    device_name: String,
}

/// The preferences model over the app's state, for one frame.
struct Model<'a> {
    state: &'a mut State,
    /// The themes to choose from: craft-libs' built-ins, PingCraft's own, the suite library's.
    themes: &'a [ThemeDef],
    tone: SkinTone,
    /// Each network's colour as drawn when it sets none (its own, in the theme's mode).
    shown_colours: BTreeMap<String, [u8; 3]>,
    words: Words,
    out: &'a mut Out,
}

impl<'a> Model<'a> {
    fn new(ui: &Ui, state: &'a mut State, out: &'a mut Out, themes: &'a [ThemeDef]) -> Self {
        let t = Tokens::get(ui.ctx());
        let networks = NETWORKS
            .iter()
            .map(|id| {
                let network = Network::from_id(id);
                (network_key(id, "sound"), network_key(id, "accent"), network_name(&network), network)
            })
            .collect();
        let shown_colours = NETWORKS
            .iter()
            .map(|id| {
                let c = t.network(&Network::from_id(id)).unwrap_or(t.muted);
                ((*id).to_owned(), [c.r(), c.g(), c.b()])
            })
            .collect();
        let device_name = state.settings.this_device().map(|d| d.name.clone()).unwrap_or_default();
        Self { tone: crate::picker::tone(ui.ctx()), state, themes, shown_colours, words: Words { networks, device_name }, out }
    }

    /// The colour a network's row shows: its own setting, else its own colour.
    fn accent_shown(&self, network: &Network) -> [u8; 3] {
        let shade = |id: &str| self.shown_colours.get(id).copied();
        match self.state.network_alerts.get(network).and_then(|a| a.accent) {
            // A network's own colour is drawn in the theme's shade of it, as everywhere else.
            Some(a) => a.network().and_then(shade).unwrap_or(a.0),
            None => shade(network.id()).unwrap_or([0x80, 0x80, 0x80]),
        }
    }

    fn set_network(&mut self, network: &Network, alerts: Alerts) {
        // Shown at once, so a second change before the backend answers builds on this one.
        self.state.network_alerts.insert(network.clone(), alerts);
        self.out.push(Command::NetworkAlerts { network: network.clone(), alerts });
    }

    fn appearance_page(&self) -> PageSpec<'_> {
        let options = [Appearance::Light, Appearance::Dark, Appearance::System]
            .into_iter()
            .map(|a| Entry::new(a.id(), AppCommand::Appearance(a).label()))
            .collect();
        PageSpec::new(
            "appearance",
            tl!("Appearance"),
            vec![
                RowSpec::new("appearance.mode", tl!("Mode"), RowKind::choice(self.state.appearance().id(), options))
                    .pills(true)
                    .keywords("light dark system theme mode"),
                RowSpec::new("appearance.light_theme", tl!("Light theme"), RowKind::Custom).keywords("theme colour color palette"),
                RowSpec::new("appearance.dark_theme", tl!("Dark theme"), RowKind::Custom).keywords("theme colour color palette"),
                RowSpec::new("appearance.import", tl!("Import a theme"), RowKind::Custom)
                    .help(tl!("A Craft theme (JSON), a base16 or base24 scheme (YAML) or a VS Code colour theme (JSON). It is kept with your Craft themes, where every Craft app finds it."))
                    .keywords("import base16 base24 vscode file"),
                RowSpec::new("appearance.just_here", tl!("Just for PingCraft"), RowKind::Bool(self.state.prefs.just_here()))
                    .help(tl!("Off: PingCraft follows the theme and mode chosen in any Craft app, and a choice here changes them there too. On: PingCraft keeps its own. A theme only PingCraft has (Catppuccin, Dracula, Gruvbox, Nord, One Dark, Solarized Light, Tokyo Night) is always PingCraft's own."))
                    .keywords("suite shared craft apps"),
            ],
        )
    }

    /// One mode's row of theme chips (craft-theme-store's `ThemeChip`, in one radio group as craft-libs' own
    /// appearance page lays them out): the themes that have that mode, the one chosen marked.
    fn theme_row(&mut self, ui: &mut Ui, dark: bool) {
        let selection = self.state.selection();
        let chosen = if dark { selection.theme.dark } else { selection.theme.light };
        let title = if dark { tl!("Dark theme") } else { tl!("Light theme") };
        let fits =
            |theme: &&ThemeDef| matches!((theme.variants, dark), (Variants::Both, _) | (Variants::DarkOnly, true) | (Variants::LightOnly, false));
        let group = RadioGroup::new(title.to_owned(), Bind::given(Some(chosen.clone()))).id_salt(("themes", dark)).show(ui, |ui, _| {
            Row::new().wrap(true).show(ui, |ui, _| {
                for theme in self.themes.iter().filter(fits) {
                    ThemeChip::new(theme, dark).can_favorite(false).id_salt(theme.id.as_str()).show(ui);
                }
            });
        });
        // The chips are swatches; the chosen theme's name is written out under them (each chip names its theme in its
        // tooltip and to a screen reader).
        if let Some(theme) = self.themes.iter().filter(fits).find(|t| t.id == chosen) {
            Text::new(theme.name.clone()).role(TypeRole::Caption).shade(Shade::Muted).show(ui);
        }
        if let Some(Some(id)) = group.output.value()
            && *id != chosen
        {
            self.state.look_changes.push(LookChange::Theme { id: id.clone(), dark });
        }
    }

    fn notifications_page(&self) -> PageSpec<'_> {
        let mut groups = vec![GroupSpec::new(
            None,
            vec![
                RowSpec::new("notifications.previews", tl!("Show message text in notifications"), RowKind::Bool(self.state.prefs.previews))
                    .help(tl!("Off: a notification says only who wrote. Kept with your account."))
                    .keywords("preview privacy"),
            ],
        )];
        for (sound, accent, name, network) in &self.words.networks {
            let own = self.state.network_alerts.get(network).copied().unwrap_or_default();
            let unset = own.accent.is_none() && Accent::of_network(network.id()).is_none();
            let mut colour = RowSpec::new(accent, tl!("Accent colour"), RowKind::Color(rgba(self.accent_shown(network)))).keywords("colour color");
            if unset {
                colour = colour.help(tl!("Not set: its chats' tags have no colour."));
            }
            groups.push(GroupSpec::new(
                Some(name.as_str()),
                vec![RowSpec::new(sound, tl!("Notification sound"), RowKind::Bool(own.sounds())).keywords("sound"), colour],
            ));
        }
        let mut page = PageSpec::new("notifications", tl!("Notifications"), Vec::new()).footnote(tl!(
            "A network's sound and colour are for its chats that do not set their own in the chat's menu. The sound is your system's own. Kept with your account, in PingCraft only."
        ));
        page.groups = groups;
        page
    }

    fn emoji_page(&self) -> PageSpec<'_> {
        let options = SkinTone::ALL.into_iter().map(|t| Entry::new(t.id(), t.label())).collect();
        PageSpec::new(
            "emoji",
            tl!("Emoji"),
            vec![
                RowSpec::new("emoji.skin_tone", tl!("Default skin tone"), RowKind::choice(self.tone.id(), options))
                    .pills(true)
                    .help(tl!("For the picker, quick reactions and :name: shortcodes. Kept with your account."))
                    .keywords("skin tone emoji"),
            ],
        )
    }

    fn security_page(&self) -> PageSpec<'_> {
        let loaded = self.state.settings.devices.is_some();
        let this = vec![
            RowSpec::new("security.device_name", tl!("This device's name"), RowKind::Text(&self.words.device_name))
                .enabled(loaded)
                .help(tl!("What your other devices call this one."))
                .keywords("device rename"),
            RowSpec::new("security.passphrase", tl!("Unlock passphrase"), RowKind::Custom).keywords("passphrase password unlock"),
            RowSpec::new("security.sign_out", tl!("Sign out"), RowKind::Custom).keywords("sign out log out"),
        ];
        let recovery = vec![RowSpec::new("security.recovery", tl!("Recovery key"), RowKind::Custom).keywords("recovery key backup")];
        let others = vec![RowSpec::new("security.devices", tl!("Your other devices"), RowKind::CustomRows).keywords("devices sessions verify")];
        let mut page = PageSpec::new("security", tl!("Security"), Vec::new());
        page.groups = vec![
            GroupSpec::new(Some(tl!("This device")), this),
            GroupSpec::new(Some(tl!("Recovery key")), recovery),
            GroupSpec::new(Some(tl!("Your other devices")), others),
        ];
        page
    }

    /// The Security page's custom rows.
    fn security_row(&mut self, ui: &mut Ui, key: &str) -> bool {
        match key {
            "security.passphrase" => {
                if self.state.passphrase == Some(true) {
                    command_button(ui, AppCommand::ChangePassphrase, Variant::SECONDARY);
                } else {
                    note(ui, tl!("PingCraft keeps its secrets in this computer's keychain, so there is no passphrase to change."));
                }
            }
            "security.sign_out" => {
                command_button(ui, AppCommand::SignOut, Variant::DANGER);
            }
            "security.recovery" => recovery_row(ui, self.state.recovery),
            "security.devices" => self.devices_rows(ui),
            _ => {}
        }
        false
    }

    /// The other devices: each one's name, when it was last seen and whether it is verified, with Verify and Sign out.
    fn devices_rows(&mut self, ui: &mut Ui) {
        let Some(devices) = self.state.settings.devices.clone() else {
            note(ui, tl!("Asking your server…"));
            return;
        };
        let others: Vec<&Device> = devices.iter().filter(|d| !d.this).collect();
        if others.is_empty() {
            note(ui, tl!("You are not signed in anywhere else."));
            return;
        }
        let busy = self.state.verification.is_some();
        for device in others {
            let name = device_name(device);
            Stack::new().id_salt(device.id.0.as_str()).show(ui, |ui, _| {
                Text::new(name.clone()).role(TypeRole::Body).show(ui);
                let seen = device.last_seen.map(|at| fmt(tl!("Last seen {time}"), &[("time", &crate::time::full(at))]));
                let trust = if device.verified { tl!("Verified") } else { tl!("Not verified") };
                let line = match seen {
                    Some(seen) => format!("{trust} · {seen}"),
                    None => trust.to_owned(),
                };
                Text::new(line).role(TypeRole::Caption).shade(Shade::Muted).show(ui);
                Row::new().show(ui, |ui, _| {
                    if !device.verified && button(ui, &fmt(tl!("Verify {device}"), &[("device", &name)]), Kind::Secondary, !busy).clicked() {
                        self.state.actions.push(SettingAction::VerifyDevice(device.id.0.clone()));
                    }
                    if button(ui, &fmt(tl!("Sign out {device}…"), &[("device", &name)]), Kind::Secondary, true).clicked() {
                        self.state.actions.push(SettingAction::SignOutDevice(device.id.0.clone()));
                    }
                });
            });
        }
    }
}

/// A craft-ui button bound to one of PingCraft's commands: its label and enabled state are the command's, and a
/// click fires it through the command board.
fn command_button(ui: &mut Ui, command: AppCommand, variant: Variant) {
    Button::command(command.id()).variant(variant).show(ui);
}

/// The recovery key's state, and what can be done about it.
fn recovery_row(ui: &mut Ui, status: RecoveryStatus) {
    let (words, command) = match status {
        RecoveryStatus::On => (tl!("Set up. This device can read your encrypted history."), Some(AppCommand::NewRecoveryKey)),
        RecoveryStatus::Off => {
            (tl!("Not set up. If you lose this device, your encrypted history is lost with it."), Some(AppCommand::NewRecoveryKey))
        }
        RecoveryStatus::NeedsKey => (
            tl!(
                "Set up, but this device does not have it yet. Enter it, or verify this device from another one. If the key is lost, you can reset it."
            ),
            Some(AppCommand::EnterRecoveryKey),
        ),
        RecoveryStatus::Unknown => (tl!("Asking your server…"), None),
    };
    note(ui, words);
    if let Some(command) = command {
        Row::new().show(ui, |ui, _| {
            command_button(ui, command, Variant::SECONDARY);
            if status == RecoveryStatus::NeedsKey {
                command_button(ui, AppCommand::VerifyThisDevice, Variant::SECONDARY);
                command_button(ui, AppCommand::ResetRecovery, Variant::DANGER);
            }
        });
    }
}

impl PreferencesModel for Model<'_> {
    fn pages(&self) -> Vec<PageSpec<'_>> {
        vec![self.appearance_page(), self.notifications_page(), self.emoji_page(), self.security_page()]
    }

    fn set(&mut self, key: &str, value: PrefValue) {
        let action = match (key, value) {
            ("appearance.mode", PrefValue::Choice(id)) => return self.state.requested.push(AppCommand::Appearance(Appearance::from_id(&id))),
            ("appearance.just_here", PrefValue::Bool(on)) => return self.state.look_changes.push(LookChange::JustHere(on)),
            ("notifications.previews", PrefValue::Bool(on)) => SettingAction::Previews(on),
            ("emoji.skin_tone", PrefValue::Choice(id)) => SettingAction::SkinTone(id),
            ("security.device_name", PrefValue::Text(typed)) => SettingAction::RenameDevice(typed),
            (key, value) => {
                let Some((id, what)) = parse_network_key(key) else { return };
                let network = id.to_owned();
                match (what, value) {
                    ("sound", PrefValue::Bool(on)) => SettingAction::NetworkSound { network, on },
                    ("accent", PrefValue::Color(Rgba([r, g, b, _]))) => SettingAction::NetworkAccent { network, colour: Accent([r, g, b]).hex() },
                    _ => return,
                }
            }
        };
        self.state.actions.push(action);
    }

    fn custom_row(&mut self, ui: &mut Ui, key: &str) -> bool {
        match key {
            "appearance.light_theme" => self.theme_row(ui, false),
            "appearance.dark_theme" => self.theme_row(ui, true),
            "appearance.import" => {
                Row::new().show(ui, |ui, _| {
                    let path = &mut self.state.settings.import_path;
                    let entered = craft_ui::text::TextField::new(tl!("Theme file").to_owned(), Bind::bound(path))
                        .placeholder(tl!("Path to a theme file").to_owned())
                        .show(ui)
                        .submitted;
                    let pressed = button(ui, tl!("Import theme"), Kind::Secondary, !self.state.settings.import_path.trim().is_empty()).clicked();
                    if (entered || pressed) && !self.state.settings.import_path.trim().is_empty() {
                        self.state.actions.push(SettingAction::ImportTheme(self.state.settings.import_path.trim().to_owned()));
                    }
                });
            }
            _ => return self.security_row(ui, key),
        }
        false
    }

    fn custom_page(&mut self, _ui: &mut Ui, _page: &str) -> bool {
        false
    }

    fn reset_page(&mut self, page: &str) {
        match page {
            // Back to the suite's theme and mode.
            "appearance" => self.state.look_changes.push(LookChange::JustHere(false)),
            "notifications" => {
                if !self.state.prefs.previews {
                    self.state.prefs.previews = true;
                    self.out.push(Command::SavePreviews(true));
                }
                for (_, _, _, network) in self.words.networks.clone() {
                    if self.state.network_alerts.get(&network).is_some_and(|a| *a != Alerts::default()) {
                        self.set_network(&network, Alerts::default());
                    }
                }
            }
            "emoji" => self.state.actions.push(SettingAction::SkinTone(SkinTone::default().id().to_owned())),
            // Nothing on the Security page has a default to go back to: each is an action, or the device's name.
            // craft-libs' page shows Reset Section on every page (PLAN.md, "craft-libs: what PingCraft lacks").
            _ => {}
        }
    }
}

/// The SETTINGS tab. Returns the colour row whose swatch was pressed, for the app to open the colour picker.
pub fn settings(ui: &mut Ui, state: &mut State, out: &mut Out, themes: &[ThemeDef]) -> Option<ColorRequest> {
    let mut page_state = std::mem::take(&mut state.settings.page);
    if let Some(notice) = state.settings.notice.clone() {
        // In line with the page under it.
        egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 0 }).show(ui, |ui| {
            Row::new().show(ui, |ui, _| {
                note(ui, &notice);
                if button(ui, tl!("Close"), Kind::Secondary, true).clicked() {
                    state.settings.notice = None;
                }
            });
        });
    }
    let mut model = Model::new(ui, state, out, themes);
    let shown = egui::Frame::new()
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| PreferencesPage::new(Bind::bound(&mut page_state)).show(ui, &mut model));
    state.settings.page = page_state;
    shown.inner.pick_color
}

/// A colour the colour picker answered for a row of the Notifications page.
pub fn picked_colour(ui: &Ui, state: &mut State, out: &mut Out, key: &str, colour: Srgba) {
    let mut model = Model::new(ui, state, out, &[]);
    model.set_color(key, colour);
}

/// Runs one of Settings' actions, from its page or fired as a command, as the page's controls would.
pub fn run_action(ctx: &egui::Context, state: &mut State, out: &mut Out, action: SettingAction) {
    match action {
        SettingAction::Previews(on) => {
            if state.prefs.previews != on {
                state.prefs.previews = on;
                out.push(Command::SavePreviews(on));
            }
        }
        SettingAction::NetworkSound { network, on } => {
            let network = Network::from_id(&network);
            let own = state.network_alerts.get(&network).copied().unwrap_or_default();
            if own.sound != Some(on) {
                set_network(state, out, network, Alerts { sound: Some(on), ..own });
            }
        }
        SettingAction::NetworkAccent { network, colour } => network_accent(ctx, state, out, &Network::from_id(&network), &colour),
        SettingAction::SkinTone(id) => crate::picker::set_tone(ctx, SkinTone::from_id(&id)),
        SettingAction::RenameDevice(typed) => {
            let current = state.settings.this_device().map(|d| d.name.clone()).unwrap_or_default();
            let sent = &mut state.settings.renamed;
            if let Some(name) = clean_name(&typed).filter(|n| *n != current && sent.as_ref() != Some(n)) {
                *sent = Some(name.clone());
                out.push(Command::RenameDevice(name));
            }
        }
        SettingAction::VerifyDevice(id) => {
            if state.verification.is_none() {
                out.push(Command::VerifyDevice(DeviceId(id)));
            }
        }
        // The app does this: it holds the suite library.
        SettingAction::ImportTheme(_) => {}
        SettingAction::SignOutDevice(id) => {
            let device = state.settings.devices.as_ref().and_then(|d| d.iter().find(|d| d.id.0 == id && !d.this)).cloned();
            if let Some(device) = device {
                state.settings.device_sign_out = Some(DeviceSignOut { device, password: Zeroizing::default(), sent: false, problem: None });
            }
        }
    }
}

/// Sets a network's alerts, shown at once so that a second change before the backend answers builds on this one.
fn set_network(state: &mut State, out: &mut Out, network: Network, alerts: Alerts) {
    state.network_alerts.insert(network.clone(), alerts);
    out.push(Command::NetworkAlerts { network, alerts });
}

/// A network's accent colour, typed or picked as `#rrggbb`. The colour the row already shows (a field committing
/// what it was given) changes nothing, and the theme's shade of a network's own colour is saved as that colour.
fn network_accent(ctx: &egui::Context, state: &mut State, out: &mut Out, network: &Network, colour: &str) {
    let Some(picked) = Accent::parse(colour).filter(|_| colour.starts_with('#')) else { return };
    let t = Tokens::get(ctx);
    let shade = |n: &Network| t.network(n).map(|c| [c.r(), c.g(), c.b()]);
    let own = state.network_alerts.get(network).copied().unwrap_or_default();
    let shown = match own.accent {
        Some(a) => a.network().and_then(|id| shade(&Network::from_id(id))).unwrap_or(a.0),
        None => shade(network).unwrap_or([t.muted.r(), t.muted.g(), t.muted.b()]),
    };
    if picked.0 == shown {
        return;
    }
    let accent = if shade(network) == Some(picked.0) { Accent::of_network(network.id()).or(Some(picked)) } else { Some(picked) };
    if accent != own.accent {
        set_network(state, out, network.clone(), Alerts { accent, ..own });
    }
}

/// The sheets Settings opens: changing the passphrase, a new recovery key, signing this or another device out.
pub fn sheets(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    passphrase_sheet(ctx, state, out);
    new_key_sheet(ctx, state, out);
    sign_out_sheet(ctx, state, out);
    device_sign_out_sheet(ctx, state, out);
}

fn passphrase_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let Some(form) = state.settings.passphrase.as_mut() else { return };
    let mut cancel = false;
    let mut submit = false;
    let mut entered = false;
    let matching = !form.new.is_empty() && *form.new == *form.again;
    let ready = !form.current.is_empty() && matching && !form.sent;
    let dismissed = crate::widgets::sheet(
        ctx,
        "change-passphrase",
        560.0,
        (tl!("Change the unlock passphrase"), tl!("This computer")),
        |ui| {
            note(ui, tl!("PingCraft asks for it when it starts, to open what it keeps on this computer. Your account's password does not change."));
            entered |= field(ui, tl!("Current passphrase"), &mut form.current, true, "").submitted;
            entered |= field(ui, tl!("New passphrase"), &mut form.new, true, "").submitted;
            entered |= field(ui, tl!("New passphrase again"), &mut form.again, true, "").submitted;
            if form.wrong {
                problem(ui, tl!("That is not the current passphrase."));
            } else if !form.again.is_empty() && !matching {
                problem(ui, tl!("The new passphrases are not the same."));
            }
            if form.sent {
                note(ui, tl!("Changing…"));
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                submit |= button(ui, tl!("Change passphrase"), Kind::Primary, ready).clicked();
            });
        },
    );
    if cancel || dismissed {
        state.settings.passphrase = None;
    } else if (submit || entered) && ready {
        form.sent = true;
        form.wrong = false;
        out.push(Command::ChangePassphrase { current: form.current.clone(), new: form.new.clone() });
    }
}

fn new_key_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let status = state.recovery;
    let Some(form) = state.settings.new_key.as_mut() else { return };
    // Rotating a key this device holds needs no password; a first key may (cross-signing), a reset always does.
    let asks_password = form.reset || status != RecoveryStatus::On;
    let ready = !form.sent && (!form.reset || !form.password.is_empty());
    let (title, action) = if form.reset {
        (tl!("Reset your recovery key?"), tl!("Reset and make a new key"))
    } else {
        (tl!("Create a new recovery key?"), tl!("Create a new key"))
    };
    let mut cancel = false;
    let mut create = false;
    let mut entered = false;
    let dismissed = crate::widgets::sheet(
        ctx,
        "new-recovery-key",
        560.0,
        (title, tl!("Your account")),
        |ui| {
            if form.reset {
                problem(
                    ui,
                    tl!(
                        "Only do this if the recovery key is lost. Messages encrypted until now can no longer be read on a new device, and your other devices need verifying again. Devices that already have their keys keep them."
                    ),
                );
                note(
                    ui,
                    tl!(
                        "PingCraft makes a new encryption identity and a new recovery key for your account. Your server asks for your account password first."
                    ),
                );
            } else if status == RecoveryStatus::On {
                note(
                    ui,
                    tl!("Your current recovery key stops working, and the new one opens your encrypted history in its place. Your history stays."),
                );
            } else {
                note(ui, tl!("Your account has no recovery key yet. With one, a new device can read your encrypted history."));
                note(ui, tl!("Your server may ask for your account password to set up encryption first."));
            }
            if asks_password {
                entered |= field(ui, tl!("Account password"), &mut form.password, true, "").submitted;
            }
            match form.refused {
                Some(RecoveryRefusal::NeedsPassword) => {
                    problem(ui, tl!("Your server asks for your account password for this."));
                }
                Some(RecoveryRefusal::WrongPassword) => {
                    problem(ui, tl!("That password was not accepted."));
                }
                Some(RecoveryRefusal::NotSupported) => {
                    problem(ui, tl!("Your server wants another way of signing in for this, which PingCraft cannot do yet."));
                }
                None => {}
            }
            if form.sent {
                note(ui, tl!("Creating…"));
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let kind = if form.reset { Kind::Danger } else { Kind::Primary };
                create = button(ui, action, kind, ready).clicked();
            });
        },
    );
    if cancel || dismissed {
        state.settings.new_key = None;
    } else if (create || entered) && ready {
        form.sent = true;
        form.refused = None;
        let password = std::mem::take(&mut form.password);
        let command = if form.reset {
            Command::ResetRecovery { password }
        } else {
            Command::NewRecoveryKey { password: (!password.is_empty()).then_some(password) }
        };
        state.recovery_forms.replacing = true;
        out.push(command);
    }
}

fn sign_out_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    if !state.settings.sign_out {
        return;
    }
    let at_risk = state.recovery != RecoveryStatus::On;
    let mut cancel = false;
    let mut sign_out = false;
    let user = state.user.clone().unwrap_or_default();
    let dismissed = crate::widgets::sheet(
        ctx,
        "sign-out",
        560.0,
        (tl!("Sign out of this device?"), &user),
        |ui| {
            note(ui, tl!("PingCraft forgets your account on this computer, with its copy of your messages and their keys."));
            if at_risk {
                problem(
                    ui,
                    tl!(
                        "This device has no recovery key set up. Its message keys go with it, and your encrypted history may be lost for good. Set up a recovery key first."
                    ),
                );
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                sign_out = button(ui, tl!("Sign out"), Kind::Danger, true).clicked();
            });
        },
    );
    if cancel || dismissed {
        state.settings.sign_out = false;
    } else if sign_out {
        state.settings.sign_out = false;
        out.push(Command::SignOut);
    }
}

fn device_sign_out_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let Some(d) = state.settings.device_sign_out.as_mut() else { return };
    let name = device_name(&d.device);
    let mut cancel = false;
    let mut submit = false;
    let mut entered = false;
    let ready = !d.password.is_empty() && !d.sent;
    let dismissed = crate::widgets::sheet(
        ctx,
        "sign-out-device",
        560.0,
        (tl!("Sign this device out?"), &name),
        |ui| {
            note(ui, tl!("It stops receiving your messages. Your server asks for your account password first."));
            entered |= field(ui, tl!("Account password"), &mut d.password, true, "").submitted;
            match &d.problem {
                Some(SignOutProblem::WrongPassword) => {
                    problem(ui, tl!("That password was not accepted."));
                }
                Some(SignOutProblem::NotSupported) => {
                    problem(
                        ui,
                        tl!("Your server wants another way of signing in for this, which PingCraft cannot do yet. Sign it out from another app."),
                    );
                }
                Some(SignOutProblem::Other(why)) => {
                    problem(ui, &fmt(tl!("It was not signed out: {why}"), &[("why", why)]));
                }
                None => {}
            }
            if d.sent {
                note(ui, tl!("Signing out…"));
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                submit |= button(ui, tl!("Sign out"), Kind::Danger, ready).clicked();
            });
        },
    );
    if cancel || dismissed {
        state.settings.device_sign_out = None;
    } else if (submit || entered) && ready {
        d.sent = true;
        d.problem = None;
        let device: DeviceId = d.device.id.clone();
        out.push(Command::SignOutDevice { device, password: std::mem::take(&mut d.password) });
    }
}

#[cfg(test)]
mod tests {
    use super::{NETWORKS, network_key, parse_network_key};

    #[test]
    fn a_networks_row_key_names_it_again() {
        for id in NETWORKS {
            assert_eq!(parse_network_key(&network_key(id, "accent")), Some((id, "accent")));
        }
        assert_eq!(parse_network_key("notifications.previews"), None);
        assert_eq!(parse_network_key("notifications.elsewhere.sound"), None);
    }
}

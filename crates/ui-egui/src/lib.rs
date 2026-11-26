//! PingCraft's interface in egui: the window's header and footer, signing in or unlocking, the inbox with the open
//! chat, and the encryption sheets.
//!
//! Layer 2: this crate sees `pingcraft-core` types only, never the Matrix SDK (ADR 0009). It talks to the Matrix
//! side through [`pingcraft_core::app::Backend`], so its tests run against a fake one.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod accounts;
mod chat;
mod commands;
mod emoji;
pub mod framework;
pub mod i18n;
mod message;
mod notify;
mod picker;
mod settings;
mod state;
mod theme;
mod time;
mod view;
mod widgets;

#[cfg(test)]
mod tests;

use craft_color_ui::ui::ColorPickerDialog;
use craft_menu::{MenuBar, MenuBarUi};
use craft_ui::contract::Tracked;
use craft_ui::dialog::{DialogHost, DialogResult};
use craft_ui::runtime::commands::{AppCatalog, CommandInput, CommandStates};
use egui::{Frame, Margin};
use pingcraft_core::app::{Backend, Command, Event};
use pingcraft_core::notify::Notifier;

use crate::commands::AppCommand;

pub use picker::SkinTone;
pub use state::{Appearance, Prefs, Screen, State, Tab};
pub use theme::Tokens;

/// The logo as drawn in the window (the design's `pingcraft-logo-128.png`).
pub(crate) const LOGO_PNG: &[u8] = include_bytes!("../../../design/assets/logo/pingcraft-logo-128.png");

/// The app icon, for the window and the task bar (`pingcraft-logo-256.png`).
pub const ICON_PNG: &[u8] = include_bytes!("../../../design/assets/logo/pingcraft-logo-256.png");

/// The whole interface over one backend.
pub struct PingCraftApp {
    backend: Box<dyn Backend>,
    pub state: State,
    /// Shows desktop notifications; without one, none are shown.
    notifier: Option<Box<dyn Notifier>>,
    /// The skin tone for emoji, as the user last chose it in the picker or the account last said.
    skin_tone: SkinTone,
    /// Whether the account has answered which tone it keeps, so a tone kept on this computer from before has been
    /// moved up to it (or set aside). The app saves this with its local copy of the tone.
    skin_tone_migrated: bool,
    /// PingCraft's commands, as registered with craft-ui's command board.
    catalog: Option<AppCatalog>,
    /// Each command's state, as last handed to the board.
    command_states: Tracked<CommandStates>,
    /// The app menu, laid out for the platform.
    menu: Tracked<MenuBar>,
    /// craft-ui's dialogs the app opens: the colour picker.
    dialogs: DialogHost,
}

impl PingCraftApp {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            state: State::default(),
            notifier: None,
            skin_tone: SkinTone::default(),
            skin_tone_migrated: false,
            catalog: commands::catalog(),
            command_states: Tracked::new(CommandStates::new()),
            menu: commands::laid_out(),
            dialogs: DialogHost::new(),
        }
    }

    /// Asks for the first frame's input from the settings given with [`Self::set_prefs`], so the window opens in the
    /// chosen mode (no light frame before a dark one). Call it once, after `set_prefs` and before the first frame.
    pub fn prime(&mut self, ctx: &egui::Context) {
        self.next_input(ctx);
    }

    /// Asks for the next frame's input (the theme's mode, PingCraft's commands and their state), which
    /// [`framework::before_input`] hands craft-ui's runtime.
    fn next_input(&mut self, ctx: &egui::Context) {
        let states = commands::states(&self.state);
        if *self.command_states != states {
            self.command_states.edit(|s| *s = states);
        }
        let mut input = CommandInput::new().states(&self.command_states);
        if let Some(catalog) = &self.catalog {
            input = input.catalog(catalog.clone());
        }
        framework::set_next(ctx, self.state.prefs.appearance, input);
    }

    /// The settings kept on this computer, for the app to save.
    pub fn prefs(&self) -> Prefs {
        self.state.prefs
    }

    /// Starts from the settings kept on this computer.
    pub fn set_prefs(&mut self, prefs: Prefs) {
        self.state.prefs = prefs;
    }

    /// Runs one of PingCraft's commands, from wherever it came. One that cannot run now does nothing.
    fn run(&mut self, command: AppCommand, out: &mut view::Out) {
        if !command.enabled(&self.state) {
            return;
        }
        let state = &mut self.state;
        match command {
            AppCommand::Inbox => state.tab = Tab::Inbox,
            AppCommand::Accounts => {
                if state.tab != Tab::Accounts {
                    view::open_accounts(state, out);
                }
            }
            AppCommand::Settings => {
                state.tab = Tab::Settings;
                out.push(Command::LoadDevices);
            }
            AppCommand::Refresh => out.push(Command::Refresh),
            AppCommand::Appearance(appearance) => state.prefs.appearance = appearance,
            AppCommand::ChangePassphrase => state.settings.passphrase = Some(settings::PassphraseForm::default()),
            AppCommand::NewRecoveryKey => state.settings.new_key = Some(false),
            AppCommand::EnterRecoveryKey => state.recovery_forms.entering = true,
            AppCommand::VerifyThisDevice => {
                if state.verification.is_none() {
                    out.push(Command::VerifyThisDevice);
                }
            }
            AppCommand::SignOut => state.settings.sign_out = true,
        }
    }

    /// Installs the interface's look and fonts on the window's context. Call it once, when the window is created,
    /// before the first frame.
    pub fn install(ctx: &egui::Context) {
        theme::install(ctx);
    }

    /// The skin tone for emoji in use, for the app to keep a local copy of. The account keeps the real one.
    pub fn skin_tone(&self) -> SkinTone {
        self.skin_tone
    }

    /// Uses this skin tone for emoji until the account says otherwise.
    pub fn set_skin_tone(&mut self, tone: SkinTone) {
        self.skin_tone = tone;
    }

    /// Starts from the local copy of the skin tone, kept from an earlier run. `migrated` when an account has
    /// already answered since (see [`Self::skin_tone_migrated`]); until then, the account is given this tone if it
    /// keeps none.
    pub fn set_local_skin_tone(&mut self, tone: SkinTone, migrated: bool) {
        self.skin_tone = tone;
        self.skin_tone_migrated = migrated;
    }

    /// Whether an account has said which skin tone it keeps, so the local copy has been moved up to it once. The
    /// app saves this next to the tone.
    pub fn skin_tone_migrated(&self) -> bool {
        self.skin_tone_migrated
    }

    /// The account said which skin tone it keeps (or that it keeps none).
    fn account_skin_tone(&mut self, ctx: &egui::Context, account: Option<&str>, out: &mut view::Out) {
        let (tone, send) = picker::account_tone(account.map(SkinTone::from_id), self.skin_tone, self.skin_tone_migrated);
        self.skin_tone_migrated = true;
        self.skin_tone = tone;
        picker::set_tone(ctx, tone);
        if send {
            out.push(Command::SaveSkinTone(tone.id().to_owned()));
        }
    }

    /// PingCraft is closing: keeps the open chat's draft (with a reply or edit in progress), as leaving the chat
    /// would. Call it before the backend is dropped, so it is handled first. Only the draft: the window waits for
    /// what is queued, and telling the chat the user stopped typing is a request not worth that wait.
    pub fn closing(&mut self) {
        for command in self.state.leaving() {
            if matches!(command, Command::SaveDraft { .. }) {
                self.backend.send(command);
            }
        }
    }

    /// Shows desktop notifications through this notifier.
    pub fn set_notifier(&mut self, notifier: Box<dyn Notifier>) {
        self.notifier = Some(notifier);
    }

    /// Hands the frame's notifications to the notifier, and opens the chats whose notifications were clicked.
    fn notifications(&mut self, ctx: &egui::Context, out: &mut view::Out) {
        let shown = std::mem::take(&mut self.state.notifications);
        let Some(notifier) = self.notifier.as_mut() else { return };
        for n in shown {
            notifier.show(n);
        }
        let clicked = notifier.clicked();
        if let Some(chat) = clicked.last().cloned()
            && self.state.screen == Screen::Inbox
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            view::open_chat_from_elsewhere(&mut self.state, chat, out);
        }
    }

    /// Draws one frame: applies what the backend said since the last one, then sends what the user did.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        theme::apply(&ctx);
        picker::set_tone(&ctx, self.skin_tone);
        self.state.focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let mut out = Vec::new();
        for event in self.backend.poll() {
            match &event {
                Event::SkinTone(account) => self.account_skin_tone(&ctx, account.as_deref(), &mut out),
                Event::RecentEmoji(recent) => picker::set_recent(&ctx, recent),
                // The next one to sign in does not see this account's.
                Event::SignedOut | Event::Locked { .. } | Event::AlreadyRunning => picker::set_recent(&ctx, &[]),
                _ => {}
            }
            self.state.apply(event);
        }
        let t = Tokens::get(&ctx);
        self.notifications(&ctx, &mut out);
        for command in framework::take_fired(&ctx).iter().filter_map(AppCommand::from_ref) {
            self.run(command, &mut out);
        }
        egui::Panel::top("menu").frame(Frame::new().fill(t.bg)).show(ui, |ui| {
            MenuBarUi::new(&self.menu).keyboard(!self.dialogs.shows_modal()).show(ui);
        });
        egui::Panel::top("header").exact_size(48.0).frame(Frame::new().fill(t.bg)).show(ui, |ui| view::header(ui, &mut self.state));
        egui::Panel::bottom("footer").exact_size(34.0).frame(Frame::new().fill(t.bg)).show(ui, |ui| view::footer(ui, &self.state));
        egui::CentralPanel::default().frame(Frame::new().fill(t.sunken).inner_margin(Margin::ZERO)).show(ui, |ui| {
            view::problem_bar(ui, &mut self.state);
            match self.state.screen.clone() {
                Screen::Starting => {}
                Screen::AlreadyRunning => view::already_running(&ctx),
                Screen::Locked { first } => view::unlock(&ctx, &mut self.state, first, &mut out),
                Screen::SignIn => view::sign_in(&ctx, &mut self.state, &mut out),
                Screen::Inbox => {
                    match self.state.tab {
                        Tab::Inbox => view::inbox(ui, &mut self.state, &mut out),
                        Tab::Accounts => accounts::accounts(ui, &mut self.state),
                        Tab::Settings => {
                            if let Some(request) = settings::settings(ui, &mut self.state, &mut out) {
                                // The eyedropper would capture the screen; a network's colour has no need of it.
                                self.dialogs.open(ColorPickerDialog::from_request(request).eyedropper(false));
                            }
                        }
                    }
                    settings::sheets(&ctx, &mut self.state, &mut out);
                    accounts::connect_sheet(&ctx, &mut self.state, &mut out);
                    accounts::remove_sheet(&ctx, &mut self.state, &mut out);
                    view::recovery_key(&ctx, &mut self.state, &mut out);
                    view::enter_recovery_key(&ctx, &mut self.state, &mut out);
                    view::verification(&ctx, &self.state, &mut out);
                }
            }
        });
        self.dialogs.run(ui);
        if let Some(DialogResult::Ok(picked)) = self.dialogs.take::<ColorPickerDialog>() {
            settings::picked_colour(ui, &mut self.state, &mut out, &picked.key, picked.picked.color);
        }
        for command in std::mem::take(&mut self.state.requested) {
            self.run(command, &mut out);
        }
        // Chosen in the picker (or on the Emoji page) this frame: kept with the account.
        let picked = picker::tone(&ctx);
        if picked != self.skin_tone {
            self.skin_tone = picked;
            out.push(Command::SaveSkinTone(picked.id().to_owned()));
        }
        out.extend(picker::take_used(&ctx).into_iter().map(Command::EmojiUsed));
        self.next_input(&ctx);
        for command in out {
            self.backend.send(command);
        }
    }
}

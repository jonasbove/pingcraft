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
mod themes;
mod time;
mod view;
mod widgets;

#[cfg(test)]
mod tests;

use craft_color_ui::ui::ColorPickerDialog;
use craft_menu::{MenuBar, MenuBarUi};
use craft_theme::{ModeChoice, ThemeId};
use craft_theme_store::{SelectionChange, SuiteClient};
use craft_ui::contract::Tracked;
use craft_ui::dialog::{DialogHost, DialogResult};
use craft_ui::runtime::commands::{AppCatalog, CommandInput, CommandStates};
use egui::{Frame, Margin};
use pingcraft_core::app::{Backend, Command, Event};
use pingcraft_core::notify::Notifier;

use crate::commands::{AppCommand, SettingAction};

pub use picker::SkinTone;
pub use state::{Appearance, LookChange, Prefs, Screen, State, Tab};
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
    /// The Craft suite's theme choice and library.
    suite: SuiteClient,
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
            suite: SuiteClient::memory(),
        }
    }

    /// Asks for the first frame's input from the settings given with [`Self::set_prefs`], so the window opens in the
    /// chosen mode (no light frame before a dark one). Call it once, after `set_prefs` and before the first frame.
    pub fn prime(&mut self, ctx: &egui::Context) {
        self.suite.sync();
        self.state.suite = self.suite.shown();
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
        let selection = self.state.selection();
        // The theme of the mode shown: with "the system's", the mode the last frame resolved.
        let dark = match selection.mode {
            ModeChoice::Dark => true,
            ModeChoice::Light => false,
            ModeChoice::System => Tokens::get(ctx).dark,
        };
        let id = if dark { selection.theme.dark } else { selection.theme.light };
        framework::set_next(ctx, selection.mode, self.theme_source(id), input);
    }

    /// Where a theme comes from: PingCraft's own, a built-in, or the suite library (craft-libs' default while a
    /// library theme loads, or when it is gone).
    fn theme_source(&self, id: ThemeId) -> framework::ThemeSource {
        // Once craft-libs ships one of PingCraft's own themes, a choice of it shows craft-libs' built-in.
        if let Some(builtin) = themes::builtin_for(&id) {
            return framework::ThemeSource::BuiltIn(builtin);
        }
        if let Some(own) = themes::own_theme(&id) {
            return framework::ThemeSource::Held(std::sync::Arc::new(own.clone()));
        }
        if id.is_builtin() {
            return framework::ThemeSource::BuiltIn(id);
        }
        match self.suite.theme(&id) {
            Some(def) => framework::ThemeSource::Held(std::sync::Arc::new(def)),
            None => framework::ThemeSource::BuiltIn(ThemeId::suite_default()),
        }
    }

    /// The suite's choice as last accepted, for the app to keep for the next start.
    pub fn suite_accepted(&self) -> craft_theme_store::Accepted {
        self.suite.accepted().clone()
    }

    /// The Craft suite's theme choice and library: on the desktop, the shared one (craft-theme-store's `SuiteDir`);
    /// without it (tests, the web build), one in memory.
    pub fn set_suite(&mut self, client: SuiteClient) {
        self.suite = client;
    }

    /// Applies a change to the look: to PingCraft's own choice when it keeps one (or for a theme only PingCraft has),
    /// else to the suite's, which every Craft app follows.
    fn change_look(&mut self, change: LookChange) {
        let prefs = &mut self.state.prefs;
        match change {
            LookChange::JustHere(true) => {
                let shown = themes::selection(&self.state.suite, &prefs.look);
                prefs.look.theme = Some(shown.theme);
                prefs.look.mode = Some(shown.mode);
            }
            LookChange::JustHere(false) => {
                prefs.look = themes::follow_suite();
            }
            LookChange::Mode(mode) => {
                if prefs.just_here() {
                    prefs.look.mode = Some(mode.mode());
                } else {
                    self.suite.change(SelectionChange { mode: Some(mode.mode()), ..SelectionChange::default() });
                }
            }
            LookChange::Theme { id, dark } => {
                // A theme only PingCraft has would be unknown to the other Craft apps: it is PingCraft's own choice.
                if prefs.just_here() || themes::own_theme(&id).is_some() {
                    let mut pair = themes::selection(&self.state.suite, &prefs.look).theme;
                    *(if dark { &mut pair.dark } else { &mut pair.light }) = id;
                    prefs.look.theme = Some(pair);
                } else if dark {
                    self.suite.change(SelectionChange::theme_dark(id));
                } else {
                    self.suite.change(SelectionChange::theme_light(id));
                }
            }
        }
    }

    /// The settings kept on this computer, for the app to save.
    pub fn prefs(&self) -> Prefs {
        self.state.prefs.clone()
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
            AppCommand::Appearance(appearance) => state.look_changes.push(LookChange::Mode(appearance)),
            AppCommand::ChangePassphrase => state.settings.passphrase = Some(settings::PassphraseForm::default()),
            AppCommand::NewRecoveryKey => state.settings.new_key = Some(settings::KeyForm::default()),
            AppCommand::ResetRecovery => state.settings.new_key = Some(settings::KeyForm { reset: true, ..Default::default() }),
            AppCommand::EnterRecoveryKey => state.recovery_forms.entering = true,
            AppCommand::VerifyThisDevice => {
                if state.verification.is_none() {
                    out.push(Command::VerifyThisDevice);
                }
            }
            AppCommand::SignOut => state.settings.sign_out = true,
            AppCommand::Search => {
                state.tab = Tab::Inbox;
                state.focus_search = true;
            }
            AppCommand::NextChat => view::step_chat(state, true, false, out),
            AppCommand::PreviousChat => view::step_chat(state, false, false, out),
            AppCommand::NextUnread => view::step_chat(state, true, true, out),
            AppCommand::PreviousUnread => view::step_chat(state, false, true, out),
            AppCommand::MarkRead => {
                if let Some(open) = &state.open {
                    out.push(Command::MarkRead(open.id.clone()));
                }
            }
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
        // The suite's choice, as another Craft app may have just changed it (one atomic load unless it did).
        self.suite.sync();
        self.state.suite = self.suite.shown();
        let mut out = Vec::new();
        for event in self.backend.poll() {
            match &event {
                Event::SkinTone(account) => self.account_skin_tone(&ctx, account.as_deref(), &mut out),
                Event::RecentEmoji(recent) => picker::set_recent(&ctx, recent),
                // Kept on this computer before it was kept with the account: an "off" moves up once.
                Event::Previews(None) if !self.state.prefs.previews => out.push(Command::SavePreviews(false)),
                // A device may have been verified: Settings lists them again.
                Event::Verification(None) if self.state.tab == Tab::Settings => out.push(Command::LoadDevices),
                // The next one to sign in does not see this account's.
                Event::SignedOut | Event::Locked { .. } | Event::AlreadyRunning => picker::set_recent(&ctx, &[]),
                _ => {}
            }
            self.state.apply(event);
        }
        let t = Tokens::get(&ctx);
        self.notifications(&ctx, &mut out);
        for fired in framework::take_fired(&ctx) {
            if let Some(command) = AppCommand::from_ref(&fired) {
                self.run(command, &mut out);
            } else if let Some(action) = SettingAction::from_ref(&fired)
                && self.state.screen == Screen::Inbox
            {
                settings::run_action(&ctx, &mut self.state, &mut out, action);
            }
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
                            let themes: Vec<craft_theme::ThemeDef> = craft_theme::builtins()
                                .iter()
                                .chain(themes::own())
                                .chain(self.suite.snapshot().index.iter().map(|entry| &entry.theme))
                                .cloned()
                                .collect();
                            if let Some(request) = settings::settings(ui, &mut self.state, &mut out, &themes) {
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
        for action in std::mem::take(&mut self.state.actions) {
            settings::run_action(&ctx, &mut self.state, &mut out, action);
        }
        for change in std::mem::take(&mut self.state.look_changes) {
            self.change_look(change);
        }
        // Escape that nothing else took (no field, menu, popup or dialog): an open chat goes back to the list.
        let back = self.state.screen == Screen::Inbox && self.state.tab == Tab::Inbox && self.state.open.is_some();
        if back && craft_ui::runtime::keys::with_window_intents(&ctx, false, |intents| intents.take(craft_ui::runtime::keys::Intent::Cancel)) {
            out.extend(self.state.leaving());
            out.push(Command::CloseChat);
            self.state.open = None;
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

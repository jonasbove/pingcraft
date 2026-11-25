//! PingCraft's interface in egui: the window's header and footer, signing in or unlocking, the inbox with the open
//! chat, and the encryption sheets.
//!
//! Layer 2: this crate sees `pingcraft-core` types only, never the Matrix SDK (ADR 0009). It talks to the Matrix
//! side through [`pingcraft_core::app::Backend`], so its tests run against a fake one.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod accounts;
mod chat;
pub mod i18n;
mod message;
mod notify;
mod state;
mod theme;
mod time;
mod view;
mod widgets;

#[cfg(test)]
mod tests;

use egui::{Frame, Margin};
use pingcraft_core::app::Backend;
use pingcraft_core::notify::Notifier;

pub use state::{Screen, State, Tab};
pub use theme::Tokens;

/// The logo as drawn in the window (the design's `pingcraft-logo-128.png`).
pub(crate) const LOGO_PNG: &[u8] = include_bytes!("../../../design/assets/logo/pingcraft-logo-128.png");

/// The app icon, for the window and the task bar (`pingcraft-logo-256.png`).
pub const ICON_PNG: &[u8] = include_bytes!("../../../design/assets/logo/pingcraft-logo-256.png");

/// The whole interface over one backend.
pub struct PingCraftApp {
    backend: Box<dyn Backend>,
    pub state: State,
    styled: bool,
    /// Shows desktop notifications; without one, none are shown.
    notifier: Option<Box<dyn Notifier>>,
}

impl PingCraftApp {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self { backend, state: State::default(), styled: false, notifier: None }
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
        if !self.styled {
            theme::install(ui.ctx());
            self.styled = true;
        }
        let ctx = ui.ctx().clone();
        self.state.focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        for event in self.backend.poll() {
            self.state.apply(event);
        }
        let t = Tokens::get(&ctx);
        let mut out = Vec::new();
        self.notifications(&ctx, &mut out);
        egui::Panel::top("header").exact_size(48.0).frame(Frame::new().fill(t.bg)).show(ui, |ui| view::header(ui, &mut self.state, &mut out));
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
                    }
                    accounts::connect_sheet(&ctx, &mut self.state, &mut out);
                    accounts::remove_sheet(&ctx, &mut self.state, &mut out);
                    view::recovery_key(&ctx, &mut self.state, &mut out);
                    view::enter_recovery_key(&ctx, &mut self.state, &mut out);
                    view::verification(&ctx, &self.state, &mut out);
                }
            }
        });
        for command in out {
            self.backend.send(command);
        }
    }
}

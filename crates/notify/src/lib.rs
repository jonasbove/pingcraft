//! PingCraft's desktop notifications, each operating system its own way, behind [`pingcraft_core::notify::Notifier`]:
//!
//! - **Linux and the BSDs** ([`freedesktop`]): the freedesktop notification spec over D-Bus
//!   (`org.freedesktop.Notifications`), with zbus, a pure-Rust D-Bus client: no libdbus, and nothing from systemd or
//!   logind, so it works on any init system. Without a session bus or a notification service it says so once on
//!   stderr and shows nothing.
//! - **macOS** ([`native`]): `UNUserNotificationCenter`. It shows notifications only for an app in a bundle with a
//!   bundle identifier, after the user allows them; PingCraft asks once, at the first notification.
//! - **Windows** ([`native`]): WinRT toasts, under PingCraft's AppUserModelID ([`APP_ID`]). Windows shows toasts
//!   only for an ID that a Start menu shortcut carries; the installer makes it.
//! - **The web:** nothing yet ([`Silent`]).
//!
//! A notification's text (a chat's name and a message preview) never goes in a log.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use std::sync::Arc;

use pingcraft_core::ChatId;
use pingcraft_core::notify::{Notification, Notifier};

/// PingCraft's application ID: the desktop entry's name on Linux, the AppUserModelID on Windows.
pub const APP_ID: &str = "io.github.jonasbove.pingcraft";
/// The name the notifications are shown under.
pub const APP_NAME: &str = "PingCraft";

/// Called when a notification was clicked, so the interface looks (in the desktop app, egui's repaint).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
pub mod freedesktop;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod native;

/// This platform's notifier.
pub fn desktop(wake: Wake) -> Box<dyn Notifier> {
    #[cfg(all(unix, not(target_os = "macos"), not(target_arch = "wasm32")))]
    return Box::new(freedesktop::Freedesktop::start(None, wake));
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    return Box::new(native::Native::new(wake));
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = wake;
        Box::new(Silent)
    }
}

/// Shows nothing: for platforms without notifications yet.
pub struct Silent;

impl Notifier for Silent {
    fn show(&mut self, _: Notification) {}

    fn clicked(&mut self) -> Vec<ChatId> {
        Vec::new()
    }
}

/// How many shown notifications are remembered for their clicks. Older ones still show; a click on one opens nothing.
const REMEMBERED: usize = 64;

/// Says once on stderr that notifications are off, and why (the platform's words, never a notification's text).
fn off_once(flag: &std::sync::atomic::AtomicBool, why: &dyn std::fmt::Display) {
    if !flag.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("pingcraft: desktop notifications are off: {why}");
    }
}

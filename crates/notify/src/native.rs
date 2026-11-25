//! macOS (`UNUserNotificationCenter`) and Windows (WinRT toasts), through notify-rust.
//!
//! Showing one blocks until the system took it, and waiting for its click blocks until the user acts, so each runs
//! on a thread of its own. At most [`crate::REMEMBERED`] wait at once; a notification shown beyond that still shows,
//! but a click on it opens nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use notify_rust::NotificationResponse;
use pingcraft_core::ChatId;
use pingcraft_core::notify::{Notification, Notifier};

use crate::{APP_NAME, REMEMBERED, Wake, off_once};

/// Notifications through the operating system's own notification center.
pub struct Native {
    click: Sender<ChatId>,
    clicks: Receiver<ChatId>,
    waiting: Arc<AtomicUsize>,
    wake: Wake,
    asked: bool,
}

static OFF: AtomicBool = AtomicBool::new(false);

impl Native {
    pub fn new(wake: Wake) -> Self {
        let (click, clicks) = channel();
        Self { click, clicks, waiting: Arc::new(AtomicUsize::new(0)), wake, asked: false }
    }
}

impl Notifier for Native {
    fn show(&mut self, n: Notification) {
        let ask = !std::mem::replace(&mut self.asked, true);
        let (click, waiting, wake) = (self.click.clone(), Arc::clone(&self.waiting), Arc::clone(&self.wake));
        let spawned = std::thread::Builder::new().name("pingcraft-notify".into()).spawn(move || {
            if ask {
                ask_permission();
            }
            let mut shown = notify_rust::Notification::new();
            shown.appname(APP_NAME).summary(&n.title).body(&n.body);
            #[cfg(target_os = "windows")]
            shown.app_id(crate::APP_ID);
            let handle = match shown.show() {
                Ok(h) => h,
                Err(e) => return off_once(&OFF, &e),
            };
            if waiting.fetch_add(1, Ordering::Relaxed) < REMEMBERED {
                let _ = handle.wait_for_response(|response: &NotificationResponse| {
                    if matches!(response, NotificationResponse::Default) && click.send(n.chat).is_ok() {
                        wake();
                    }
                });
            }
            waiting.fetch_sub(1, Ordering::Relaxed);
        });
        if let Err(e) = spawned {
            off_once(&OFF, &e);
        }
    }

    fn clicked(&mut self) -> Vec<ChatId> {
        self.clicks.try_iter().collect()
    }
}

/// macOS asks the user once whether PingCraft may show notifications.
#[cfg(target_os = "macos")]
fn ask_permission() {
    if let Err(e) = notify_rust::request_auth_blocking() {
        off_once(&OFF, &format!("{e:?}"));
    }
}

#[cfg(target_os = "windows")]
fn ask_permission() {}

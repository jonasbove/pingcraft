//! Linux and the BSDs: `org.freedesktop.Notifications` on the session bus, through zbus.
//!
//! A worker thread sends the notifications, so a notification service that is slow to answer never holds up the
//! interface; a second thread listens for clicks (the `default` action). Both end when the [`Freedesktop`] is
//! dropped and the bus goes away.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use pingcraft_core::ChatId;
use pingcraft_core::notify::{Notification, Notifier};
use zbus::zvariant::Value;

use crate::{APP_ID, APP_NAME, REMEMBERED, Wake, off_once};

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

/// Notifications through the freedesktop notification service.
pub struct Freedesktop {
    to_show: Sender<Notification>,
    clicks: Receiver<ChatId>,
}

/// The notifications shown, newest last, by the service's ID, for their clicks.
type Shown = Arc<Mutex<VecDeque<(u32, ChatId)>>>;

impl Freedesktop {
    /// Starts the worker on the session bus, or on the bus at `address` (tests).
    pub fn start(address: Option<String>, wake: Wake) -> Self {
        let (to_show, queue) = channel();
        let (click, clicks) = channel();
        let spawned = std::thread::Builder::new().name("pingcraft-notify".into()).spawn(move || run(address, queue, click, wake));
        if let Err(e) = spawned {
            off_once(&OFF, &e);
        }
        Self { to_show, clicks }
    }
}

impl Notifier for Freedesktop {
    fn show(&mut self, notification: Notification) {
        // A worker that is gone has said why already.
        let _ = self.to_show.send(notification);
    }

    fn clicked(&mut self) -> Vec<ChatId> {
        self.clicks.try_iter().collect()
    }
}

/// Whether "notifications are off" was said already.
static OFF: AtomicBool = AtomicBool::new(false);

fn connect(address: Option<&str>) -> zbus::Result<zbus::blocking::Connection> {
    match address {
        Some(a) => zbus::blocking::connection::Builder::address(a)?.build(),
        None => zbus::blocking::Connection::session(),
    }
}

fn run(address: Option<String>, queue: Receiver<Notification>, click: Sender<ChatId>, wake: Wake) {
    let proxy = match connect(address.as_deref()).and_then(|c| NotificationsProxyBlocking::new(&c)) {
        Ok(p) => p,
        Err(e) => {
            off_once(&OFF, &format!("no session bus ({e})"));
            // Take what is sent and drop it, so senders never notice.
            for _ in queue {}
            return;
        }
    };
    let shown: Shown = Arc::new(Mutex::new(VecDeque::new()));
    listen(&proxy, Arc::clone(&shown), click, wake);
    for n in queue {
        let hints = hints(n.sound);
        // "default" is the click on the notification itself; its label is not shown.
        match proxy.notify(APP_NAME, 0, APP_ID, &escape(&n.title), &escape(&n.body), &["default", "Open"], hints, -1) {
            Ok(id) => {
                if let Ok(mut s) = shown.lock() {
                    s.push_back((id, n.chat));
                    while s.len() > REMEMBERED {
                        s.pop_front();
                    }
                }
            }
            Err(e) => off_once(&OFF, &format!("the notification service did not answer ({e})")),
        }
    }
}

/// The hints sent with a notification. With `sound`, the desktop's own sound for an instant message (the
/// freedesktop sound naming spec's `message-new-instant`, from the user's sound theme: no sound file of
/// PingCraft's); without, a request for silence, which services that play a sound of their own honour.
fn hints(sound: bool) -> HashMap<&'static str, Value<'static>> {
    let mut hints = HashMap::new();
    hints.insert("desktop-entry", Value::from(APP_ID));
    hints.insert("category", Value::from("im.received"));
    if sound {
        hints.insert("sound-name", Value::from("message-new-instant"));
    } else {
        hints.insert("suppress-sound", Value::from(true));
    }
    hints
}

/// Listens for clicks on PingCraft's notifications, on a thread of its own.
fn listen(proxy: &NotificationsProxyBlocking<'static>, shown: Shown, click: Sender<ChatId>, wake: Wake) {
    let Ok(signals) = proxy.receive_action_invoked() else { return };
    let spawned = std::thread::Builder::new().name("pingcraft-notify-clicks".into()).spawn(move || {
        for signal in signals {
            let Ok(args) = signal.args() else { continue };
            if args.action_key != "default" {
                continue;
            }
            let chat = shown.lock().ok().and_then(|s| s.iter().find(|(id, _)| *id == args.id).map(|(_, c)| c.clone()));
            if let Some(chat) = chat {
                if click.send(chat).is_err() {
                    return;
                }
                wake();
            }
        }
    });
    if let Err(e) = spawned {
        off_once(&OFF, &e);
    }
}

/// The spec lets a service read the body as markup: names and messages are shown as written.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{Freedesktop, escape, hints};
    use pingcraft_core::ChatId;
    use pingcraft_core::notify::{Notification, Notifier};
    use std::sync::Arc;
    use zbus::zvariant::Value;

    #[test]
    fn without_a_bus_notifications_are_dropped_quietly() {
        let mut n = Freedesktop::start(Some("unix:path=/nonexistent/pingcraft-test-bus".into()), Arc::new(|| {}));
        for _ in 0..3 {
            n.show(Notification { chat: ChatId("!a".into()), title: "Book club".into(), body: "Ana: hi".into(), sound: true });
        }
        assert!(n.clicked().is_empty());
    }

    /// With sound, the desktop's own instant-message sound is asked for; without, silence.
    #[test]
    fn the_sound_is_the_desktops_own_or_none() {
        let on = hints(true);
        assert_eq!(on.get("sound-name"), Some(&Value::from("message-new-instant")));
        assert!(!on.contains_key("suppress-sound"));
        let off = hints(false);
        assert_eq!(off.get("suppress-sound"), Some(&Value::from(true)));
        assert!(!off.contains_key("sound-name"));
        assert_eq!(off.get("category"), Some(&Value::from("im.received")));
    }

    #[test]
    fn markup_in_names_and_messages_is_shown_as_written() {
        assert_eq!(escape("<b>Tom & Ana</b>"), "&lt;b&gt;Tom &amp; Ana&lt;/b&gt;");
    }
}

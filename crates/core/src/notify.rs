//! Desktop notifications, as the interface asks for them. Each operating system shows them its own way behind
//! [`Notifier`] (the desktop app's `pingcraft-notify`); the interface decides when, and tests use a fake.

use crate::ChatId;

/// One notification: a new message in a chat.
#[derive(Clone, PartialEq, Eq)]
pub struct Notification {
    /// The chat to open when the notification is clicked.
    pub chat: ChatId,
    /// The chat's name.
    pub title: String,
    /// The sender and a short preview of the message.
    pub body: String,
}

/// Never shows the title or body: they are a chat's name and a message's text.
impl std::fmt::Debug for Notification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notification").field("chat", &self.chat).finish_non_exhaustive()
    }
}

/// Shows notifications, and says which ones were clicked.
pub trait Notifier {
    /// Shows a notification. Never blocks, and fails quietly: notifications are a courtesy.
    fn show(&mut self, notification: Notification);
    /// The chats whose notifications were clicked since the last call, oldest first. Never blocks.
    fn clicked(&mut self) -> Vec<ChatId>;
}

#[cfg(test)]
mod tests {
    use super::Notification;
    use crate::ChatId;

    #[test]
    fn debug_never_shows_the_text() {
        let n = Notification { chat: ChatId("!a".into()), title: "Secret club".into(), body: "Ana: private words".into() };
        let shown = format!("{n:?}");
        assert!(!shown.contains("Secret") && !shown.contains("private"), "{shown}");
    }
}

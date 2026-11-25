//! When to show a desktop notification, and what it says. Pure functions over what the interface knows, so they
//! are tested without a window or an operating system.
//!
//! Two ways a message arrives, so each is counted once:
//! - **A chat that is not open:** its inbox row comes again with more unread messages and a newer last message.
//!   A row whose last message only now filled in (the inbox works previews out after it loads) has no more unread
//!   messages than before, so it does not count.
//! - **The open chat:** it is read as messages arrive, so its unread count never rises; a new last message from
//!   someone else counts instead, unless the window has focus. The first list after opening a chat never counts.

use pingcraft_core::notify::Notification;
use pingcraft_core::{Chat, Message, MessageBody};

use crate::i18n::fmt;
use crate::tl;

/// The longest preview a notification shows, in characters.
const PREVIEW: usize = 120;

/// A notification for a change to a chat that is not open, if it brings a new message to tell about.
pub fn for_inbox_change(before: Option<&Chat>, after: &Chat) -> Option<Notification> {
    if after.muted.is_some() {
        return None;
    }
    let message = after.last_message.as_ref().filter(|m| !m.own)?;
    let more_unread = after.unread > before.map_or(0, |b| b.unread);
    let newer = before.and_then(|b| b.last_message.as_ref()).is_none_or(|b| b.at < message.at);
    (more_unread && newer).then(|| notification(after, &message.sender, &message.body))
}

/// A notification for a new list of the open chat's messages, if its newest is new and from someone else and the
/// window does not have focus.
pub fn for_open_chat(before: &[Message], after: &[Message], chat: Option<&Chat>, focused: bool) -> Option<Notification> {
    let chat = chat?;
    if focused || before.is_empty() || chat.muted.is_some() {
        return None;
    }
    let newest = after.last().filter(|m| !m.own)?;
    if before.iter().any(|m| m.id == newest.id) {
        return None;
    }
    Some(notification(chat, &newest.sender, &newest.body))
}

fn notification(chat: &Chat, sender: &str, body: &MessageBody) -> Notification {
    let line = crate::view::preview_text(body);
    let mut preview: String = line.chars().take(PREVIEW).collect();
    if line.chars().count() > PREVIEW {
        preview.push('…');
    }
    Notification { chat: chat.id.clone(), title: chat.name.clone(), body: fmt(tl!("{sender}: {text}"), &[("sender", sender), ("text", &preview)]) }
}

#[cfg(test)]
mod tests {
    use super::{for_inbox_change, for_open_chat};
    use pingcraft_core::{Chat, ChatId, Message, MessageBody, MessageId, MessagePreview, Mute, Network, SendState, Timestamp};

    fn chat(unread: u32, last: Option<(&str, bool, u64)>) -> Chat {
        Chat {
            id: ChatId("!book".into()),
            account: None,
            network: Network::from_id("signal"),
            name: "Book club".into(),
            last_message: last.map(|(sender, own, at)| MessagePreview {
                sender: sender.into(),
                own,
                body: MessageBody::Text("See you at eight".into()),
                at: Timestamp(at),
            }),
            unread,
            encrypted: true,
            pinned: false,
            muted: None,
            marked_unread: false,
            draft: None,
        }
    }

    fn message(id: &str, own: bool) -> Message {
        Message::new(MessageId(id.into()), "Lars Holm", own, Timestamp(1), MessageBody::Text(format!("message {id}")), SendState::Sent)
    }

    #[test]
    fn a_new_unread_message_in_a_closed_chat_notifies() {
        let n = for_inbox_change(Some(&chat(1, Some(("Lars", false, 10)))), &chat(2, Some(("Lars", false, 20)))).unwrap();
        assert_eq!(n.chat.0, "!book");
        assert_eq!(n.title, "Book club");
        assert_eq!(n.body, "Lars: See you at eight");
        assert!(for_inbox_change(None, &chat(1, Some(("Lars", false, 20)))).is_some(), "a chat that just appeared");
    }

    #[test]
    fn what_is_not_a_new_message_from_someone_else_does_not_notify() {
        let cases = [
            ("a preview filling in after loading", chat(3, None), chat(3, Some(("Lars", false, 20)))),
            ("the user's own message", chat(0, Some(("Lars", false, 10))), chat(1, Some(("Ana", true, 20)))),
            ("read elsewhere", chat(2, Some(("Lars", false, 10))), chat(0, Some(("Lars", false, 20)))),
            ("pinned, a draft kept, or opened", chat(1, Some(("Lars", false, 10))), chat(1, Some(("Lars", false, 10)))),
        ];
        for (what, before, after) in cases {
            assert!(for_inbox_change(Some(&before), &after).is_none(), "{what}");
        }
        let mut muted = chat(2, Some(("Lars", false, 20)));
        muted.muted = Some(Mute::UntilUnmuted);
        assert!(for_inbox_change(Some(&chat(1, Some(("Lars", false, 10)))), &muted).is_none(), "muted");
    }

    #[test]
    fn the_open_chat_notifies_only_without_focus_and_for_new_messages() {
        let c = chat(0, None);
        let before = [message("1", false)];
        let after = [message("1", false), message("2", false)];
        let n = for_open_chat(&before, &after, Some(&c), false).unwrap();
        assert_eq!(n.body, "Lars Holm: message 2");
        assert!(for_open_chat(&before, &after, Some(&c), true).is_none(), "the window has focus");
        assert!(for_open_chat(&[], &after, Some(&c), false).is_none(), "the first list after opening");
        assert!(for_open_chat(&after, &after, Some(&c), false).is_none(), "nothing new");
        assert!(for_open_chat(&before, &[message("1", false), message("2", true)], Some(&c), false).is_none(), "the user's own");
        let mut muted = c.clone();
        muted.muted = Some(Mute::UntilUnmuted);
        assert!(for_open_chat(&before, &after, Some(&muted), false).is_none(), "muted");
    }
}

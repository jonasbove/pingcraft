//! The inbox: all of a user's chats from all their accounts, together in one list.

use crate::{Chat, ChatId};

/// Which chats the inbox shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Unread,
}

/// All of a user's chats, newest activity first. Ties keep a stable order by name, then ID, so the list does not
/// jump around when two chats share a timestamp.
#[derive(Debug, Clone, Default)]
pub struct Inbox {
    chats: Vec<Chat>,
}

impl Inbox {
    pub fn new(chats: impl IntoIterator<Item = Chat>) -> Self {
        let mut inbox = Self { chats: Vec::new() };
        for chat in chats {
            inbox.upsert(chat);
        }
        inbox
    }

    /// Adds a chat, or replaces the one with the same ID, and keeps the order.
    pub fn upsert(&mut self, chat: Chat) {
        self.chats.retain(|c| c.id != chat.id);
        let at = self.chats.partition_point(|c| Self::before(c, &chat));
        self.chats.insert(at, chat);
    }

    pub fn remove(&mut self, id: &ChatId) -> Option<Chat> {
        let i = self.chats.iter().position(|c| &c.id == id)?;
        Some(self.chats.remove(i))
    }

    pub fn get(&self, id: &ChatId) -> Option<&Chat> {
        self.chats.iter().find(|c| &c.id == id)
    }

    pub fn len(&self) -> usize {
        self.chats.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chats.is_empty()
    }

    /// The chats to show for a filter and a search. The search matches chat names, ignoring case; an empty search
    /// matches everything.
    pub fn visible<'a>(&'a self, filter: Filter, search: &'a str) -> impl Iterator<Item = &'a Chat> + 'a {
        let needle = search.trim().to_lowercase();
        self.chats.iter().filter(move |c| {
            let passes_filter = match filter {
                Filter::All => true,
                Filter::Unread => c.unread > 0,
            };
            passes_filter && (needle.is_empty() || c.name.to_lowercase().contains(&needle))
        })
    }

    /// The number of chats with unread messages, for the window title and the filter's count.
    pub fn unread_chats(&self) -> usize {
        self.chats.iter().filter(|c| c.unread > 0).count()
    }

    /// Whether `a` belongs before `b`: newer activity first, then name, then ID.
    fn before(a: &Chat, b: &Chat) -> bool {
        (std::cmp::Reverse(a.last_activity()), &a.name, &a.id) < (std::cmp::Reverse(b.last_activity()), &b.name, &b.id)
    }
}

#[cfg(test)]
mod tests {
    use super::{Filter, Inbox};
    use crate::{Chat, ChatId, MessagePreview, Network, Timestamp};

    fn chat(id: &str, name: &str, at: u64, unread: u32) -> Chat {
        Chat {
            id: ChatId(id.into()),
            account: None,
            network: Network::from_id("telegram"),
            name: name.into(),
            last_message: (at > 0).then(|| MessagePreview { sender: "x".into(), text: "y".into(), at: Timestamp(at) }),
            unread,
            encrypted: true,
        }
    }

    fn names<'a>(it: impl Iterator<Item = &'a Chat>) -> Vec<&'a str> {
        it.map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn newest_activity_comes_first_and_ties_are_stable() {
        let inbox = Inbox::new([chat("1", "Book club", 10, 0), chat("2", "Ana", 30, 0), chat("3", "Lars", 10, 0), chat("4", "Empty", 0, 0)]);
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Ana", "Book club", "Lars", "Empty"]);
    }

    #[test]
    fn upsert_replaces_and_moves_a_chat() {
        let mut inbox = Inbox::new([chat("1", "Book club", 10, 0), chat("2", "Ana", 30, 0)]);
        inbox.upsert(chat("1", "Book club", 40, 1));
        assert_eq!(inbox.len(), 2);
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Book club", "Ana"]);
        assert_eq!(inbox.get(&ChatId("1".into())).map(|c| c.unread), Some(1));
    }

    #[test]
    fn filter_and_search_combine() {
        let inbox = Inbox::new([chat("1", "Book club", 10, 2), chat("2", "Ana Ortiz", 30, 0), chat("3", "Family", 20, 5)]);
        assert_eq!(names(inbox.visible(Filter::Unread, "")), ["Family", "Book club"]);
        assert_eq!(names(inbox.visible(Filter::All, "  ORT ")), ["Ana Ortiz"]);
        assert_eq!(names(inbox.visible(Filter::Unread, "ana")), Vec::<&str>::new());
        assert_eq!(inbox.unread_chats(), 2);
    }

    #[test]
    fn remove_takes_a_chat_out() {
        let mut inbox = Inbox::new([chat("1", "A", 1, 0)]);
        assert!(inbox.remove(&ChatId("1".into())).is_some());
        assert!(inbox.remove(&ChatId("1".into())).is_none());
        assert!(inbox.is_empty());
    }
}

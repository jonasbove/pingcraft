//! The inbox: all of a user's chats from all their accounts, together in one list.

use crate::{Chat, ChatId};

/// One change to the inbox, as the Matrix adapter reports it. The adapter sends a [`InboxChange::Reset`] first,
/// then changes; [`Inbox::apply`] keeps the order, so the adapter does not have to.
// A chat is a few hundred bytes and changes come one at a time over a channel, so boxing `Upsert` would only add an
// allocation per change.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxChange {
    /// Every chat, replacing what was there.
    Reset(Vec<Chat>),
    /// A chat is new or changed.
    Upsert(Chat),
    /// A chat left the inbox (the user left it, or it was upgraded or forgotten).
    Remove(ChatId),
}

/// Which chats the inbox shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Unread,
    /// The archived chats, which the other filters leave out.
    Archived,
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

    /// Applies one change from the adapter.
    pub fn apply(&mut self, change: InboxChange) {
        match change {
            InboxChange::Reset(chats) => *self = Self::new(chats),
            InboxChange::Upsert(chat) => self.upsert(chat),
            InboxChange::Remove(id) => {
                self.remove(&id);
            }
        }
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
                Filter::All => !c.is_archived(),
                Filter::Unread => !c.is_archived() && c.is_unread(),
                Filter::Archived => c.is_archived(),
            };
            passes_filter && (needle.is_empty() || c.name.to_lowercase().contains(&needle))
        })
    }

    /// The number of chats with unread messages, for the window title and the filter's count. Archived chats are
    /// left out: they are out of the inbox.
    pub fn unread_chats(&self) -> usize {
        self.chats.iter().filter(|c| !c.is_archived() && c.is_unread()).count()
    }

    /// The number of chats in the inbox, archived ones left out.
    pub fn in_inbox(&self) -> usize {
        self.chats.iter().filter(|c| !c.is_archived()).count()
    }

    /// The number of archived chats.
    pub fn archived(&self) -> usize {
        self.chats.iter().filter(|c| c.is_archived()).count()
    }

    /// Whether `a` belongs before `b`: pinned chats first, then newer activity, then name, then ID.
    fn before(a: &Chat, b: &Chat) -> bool {
        let key = |c: &'_ Chat| (!c.pinned, std::cmp::Reverse(c.last_activity()));
        (key(a), &a.name, &a.id) < (key(b), &b.name, &b.id)
    }
}

#[cfg(test)]
mod tests {
    use super::{Filter, Inbox, InboxChange};
    use crate::{Chat, ChatId, MessageBody, MessagePreview, Network, Timestamp};

    fn chat(id: &str, name: &str, at: u64, unread: u32) -> Chat {
        Chat {
            id: ChatId(id.into()),
            account: None,
            network: Network::from_id("telegram"),
            name: name.into(),
            last_message: (at > 0).then(|| MessagePreview { sender: "x".into(), own: false, body: MessageBody::Text("y".into()), at: Timestamp(at) }),
            unread,
            encrypted: true,
            pinned: false,
            muted: None,
            marked_unread: false,
            archived: None,
            draft: None,
            draft_for: None,
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
    fn pinned_chats_come_first_and_marked_ones_count_as_unread() {
        let mut old = chat("1", "Old but pinned", 1, 0);
        old.pinned = true;
        let mut marked = chat("2", "Marked", 5, 0);
        marked.marked_unread = true;
        let inbox = Inbox::new([chat("3", "Newest", 9, 0), marked, old]);
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Old but pinned", "Newest", "Marked"]);
        assert_eq!(names(inbox.visible(Filter::Unread, "")), ["Marked"]);
        assert_eq!(inbox.unread_chats(), 1);
    }

    /// An archived chat leaves the inbox and its counts, and comes back with a newer message unless it is muted.
    #[test]
    fn archived_chats_leave_the_inbox_until_a_new_message_unless_muted() {
        let mut quiet = chat("1", "Quiet", 10, 3);
        quiet.archived = Some(Timestamp(10));
        let mut muted = chat("2", "Muted", 50, 1);
        muted.archived = Some(Timestamp(20));
        muted.muted = Some(crate::Mute::UntilUnmuted);
        let mut empty = chat("4", "No messages", 0, 0);
        empty.archived = Some(Timestamp(5));
        let mut inbox = Inbox::new([quiet.clone(), muted, chat("3", "Busy", 30, 1), empty]);
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Busy"]);
        assert_eq!(names(inbox.visible(Filter::Unread, "")), ["Busy"]);
        assert_eq!(names(inbox.visible(Filter::Archived, "")), ["Muted", "Quiet", "No messages"]);
        assert_eq!((inbox.in_inbox(), inbox.unread_chats(), inbox.archived()), (1, 1, 3));
        // A message after it was archived brings it back.
        let mut back = chat("1", "Quiet", 11, 4);
        back.archived = quiet.archived;
        assert!(!back.is_archived());
        inbox.upsert(back);
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Busy", "Quiet"]);
        assert_eq!(names(inbox.visible(Filter::Archived, "qu")), Vec::<&str>::new());
    }

    #[test]
    fn remove_takes_a_chat_out() {
        let mut inbox = Inbox::new([chat("1", "A", 1, 0)]);
        assert!(inbox.remove(&ChatId("1".into())).is_some());
        assert!(inbox.remove(&ChatId("1".into())).is_none());
        assert!(inbox.is_empty());
    }

    #[test]
    fn changes_apply_in_order() {
        let mut inbox = Inbox::default();
        inbox.apply(InboxChange::Reset(vec![chat("1", "A", 1, 0), chat("2", "B", 2, 0)]));
        inbox.apply(InboxChange::Upsert(chat("3", "C", 3, 1)));
        inbox.apply(InboxChange::Remove(ChatId("1".into())));
        assert_eq!(names(inbox.visible(Filter::All, "")), ["C", "B"]);
        inbox.apply(InboxChange::Reset(vec![chat("9", "Z", 1, 0)]));
        assert_eq!(names(inbox.visible(Filter::All, "")), ["Z"]);
    }
}

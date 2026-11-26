//! PingCraft's domain model, in the words of `GLOSSARY.md`: networks, accounts, chats, messages and the inbox.
//!
//! Layer: lowest. Nothing here depends on the Matrix SDK or on egui (ADR 0009), so the interface can be tested
//! without a server, and SDK upgrades stay in `pingcraft-matrix`. Text that came from a network (names, message
//! bodies) is held as given; nothing here trusts it to be short or well formed.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub mod app;
pub mod bridge;
pub mod encryption;
mod inbox;
pub mod notify;

pub use inbox::{Filter, Inbox, InboxChange};

/// A chat service a user can connect, such as Telegram. Matrix itself is also a network.
///
/// Networks are open-ended (a bridge provider can offer new ones), so this is an identifier, not an enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Network(String);

impl Network {
    /// The network a chat belongs to when no bridge claims it.
    pub fn matrix() -> Self {
        Self("matrix".to_owned())
    }

    /// A network by the identifier its bridge reports (for mautrix: `protocol.id` in the bridge-info state).
    /// An empty identifier means no bridge claimed the chat, so it is Matrix.
    pub fn from_id(id: &str) -> Self {
        let id = id.trim();
        if id.is_empty() { Self::matrix() } else { Self(id.to_ascii_lowercase()) }
    }

    pub fn id(&self) -> &str {
        &self.0
    }

    pub fn is_matrix(&self) -> bool {
        self.0 == "matrix"
    }

    /// How likely the network is to restrict or ban an account for being connected through a bridge, as decided
    /// in its ADR (0005 to 0008). `None` for Matrix and for networks PingCraft has not assessed. Discord's is for
    /// logging in as yourself; its bot login is low.
    pub fn risk_level(&self) -> Option<RiskLevel> {
        match self.0.as_str() {
            "telegram" => Some(RiskLevel::Low),
            "signal" => Some(RiskLevel::Medium),
            "whatsapp" => Some(RiskLevel::MediumHigh),
            "discord" => Some(RiskLevel::High),
            _ => None,
        }
    }

    /// How the network's own app draws a symbol that is text by default in Unicode, such as ☺ U+263A or ❤ U+2764,
    /// when it comes without the emoji variation selector (U+FE0F): PingCraft draws a chat's messages the way the
    /// people in it see them on that network. What each app does, and how sure that is: PLAN.md, "Messages and
    /// chats" 1. Matrix, and networks PingCraft has not looked at, follow Unicode.
    pub fn emoji_presentation(&self) -> EmojiPresentation {
        match self.0.as_str() {
            "telegram" => EmojiPresentation::EmojiFirst { arrows: true },
            "signal" | "whatsapp" => EmojiPresentation::EmojiFirst { arrows: false },
            _ => EmojiPresentation::Unicode,
        }
    }
}

/// Whether a symbol that is text by default in Unicode (☺ U+263A, ❤ U+2764, ✔ U+2714) is drawn as an emoji when it
/// comes without the emoji variation selector (U+FE0F), as a network's own app does ([`Network::emoji_presentation`]).
/// In every case U+FE0F makes such a symbol an emoji, U+FE0E keeps it text, and digits, `#` and `*` are text unless
/// they are a keycap (with U+20E3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmojiPresentation {
    /// As Unicode says: text unless U+FE0F follows.
    Unicode,
    /// An emoji whenever it has an emoji form, except ©, ® and ™, which stay text, and the arrows (↔ U+2194, ➡ U+27A1
    /// and the like), which are emoji only with `arrows`.
    EmojiFirst { arrows: bool },
}

/// How likely a network is to restrict or ban an account for being connected through a bridge. Shown to the user
/// before they connect one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    MediumHigh,
    High,
}

/// Whether PingCraft is keeping up with the server, for the line at the top of the inbox.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnectionState {
    /// Started, the first answer has not come yet.
    #[default]
    Connecting,
    /// Syncing.
    Online,
    /// The server cannot be reached; PingCraft keeps trying and shows what it has.
    Offline,
    /// Stopped on purpose (signing out, closing).
    Stopped,
    /// Syncing stopped with an error, in the adapter's words, for the log.
    Error(String),
}

/// A point in time, in milliseconds since the Unix epoch (Matrix's `origin_server_ts`). Turning it into a
/// calendar date is the interface's job, through `chrono` (house rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Timestamp(pub u64);

/// Identifies one account: a login on one network. It is the bridge's own ID for the login, so it is unique within
/// its network only (two bridges may both use a phone number); always keep it with its [`Network`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub String);

/// One login on one network. A user can have several, including two on the same network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub network: Network,
    /// How the account names itself on its network, e.g. a phone number or a username, as the bridge reports it.
    pub name: String,
    pub state: AccountState,
}

/// Whether an account's bridge is currently carrying its messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountState {
    Connected,
    Connecting,
    /// The network ended the session, or the bridge lost it; the user has to log in again.
    LoggedOut,
    /// The bridge reported a problem in its own words.
    Error(String),
}

/// Identifies one chat. For Matrix this is the room ID; the interface never shows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChatId(pub String);

/// One conversation as seen through one account. A group two of a user's accounts are both in is two chats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub id: ChatId,
    /// The account this chat is seen through; `None` for a chat on the user's own Matrix account.
    pub account: Option<AccountId>,
    pub network: Network,
    pub name: String,
    pub last_message: Option<MessagePreview>,
    pub unread: u32,
    /// Whether messages in this chat are end-to-end encrypted on the Matrix side.
    pub encrypted: bool,
    /// Pinned to the top of the inbox (the Matrix favourite tag, so other apps see it too).
    pub pinned: bool,
    /// Muted: its messages make no notification. It still counts as unread.
    pub muted: Option<Mute>,
    /// The user marked it as unread.
    pub marked_unread: bool,
    /// What the user typed and has not sent yet, kept in the encrypted store.
    pub draft: Option<String>,
    /// The message the draft replies to or edits; `None` for a new message.
    pub draft_for: Option<DraftFor>,
    /// When the user archived it (PingCraft's own room account data). Whether it is still archived is
    /// [`Chat::is_archived`]: a newer message brings it back unless it is muted.
    pub archived: Option<Timestamp>,
    /// The chat's own notification sound and accent colour; what it leaves unset follows its network's.
    pub alerts: Alerts,
}

/// What a draft is for, besides a new message: a reply to a message, or an edit of one of the user's own, by the
/// message's [`Message::event_id`] (a [`MessageId`] does not last from one run to the next; the event ID does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftFor {
    Reply(String),
    Edit(String),
}

/// The accent colours a chat or a network can take: the network colours, by the network they belong to by default
/// (PLAN.md, "Later": network colours). Saved by [`Accent::id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accent {
    Telegram,
    Signal,
    WhatsApp,
    Discord,
}

impl Accent {
    pub const ALL: [Self; 4] = [Self::Telegram, Self::Signal, Self::WhatsApp, Self::Discord];

    /// The name it is saved under, which is also the network whose colour it is. Never change one.
    pub fn id(self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Signal => "signal",
            Self::WhatsApp => "whatsapp",
            Self::Discord => "discord",
        }
    }

    /// The colour saved under `id`; `None` for anything else (a saved value is never trusted).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// How a chat's or a network's new messages are announced: a notification sound (the operating system's own) on or
/// off, and an accent colour. `None` follows the level above: a chat its network's, a network the default (sound on,
/// its own colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Alerts {
    pub sound: Option<bool>,
    pub accent: Option<Accent>,
}

impl Alerts {
    /// Whether a notification sounds when there are no settings at all.
    pub const DEFAULT_SOUND: bool = true;

    /// These settings, with what they leave unset taken from `above`.
    pub fn over(self, above: Alerts) -> Alerts {
        Alerts { sound: self.sound.or(above.sound), accent: self.accent.or(above.accent) }
    }

    /// Whether a notification sounds, once every level has been applied.
    pub fn sounds(self) -> bool {
        self.sound.unwrap_or(Self::DEFAULT_SOUND)
    }
}

/// How long a chat stays muted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mute {
    /// Until this time; PingCraft turns it back on then, while it runs.
    Until(Timestamp),
    /// Until the user turns it back on.
    UntilUnmuted,
}

impl Chat {
    /// Whether the inbox shows the chat as unread: it has unread messages, or the user marked it.
    pub fn is_unread(&self) -> bool {
        self.unread > 0 || self.marked_unread
    }

    /// Whether the chat is out of the inbox, in the archive: the user archived it, and since then no message has
    /// come, or it is muted (a muted chat stays archived whatever arrives).
    pub fn is_archived(&self) -> bool {
        self.archived.is_some_and(|at| self.muted.is_some() || self.last_activity() <= at)
    }

    /// When the chat last had a message, for sorting the inbox. A chat without messages sorts last.
    pub fn last_activity(&self) -> Timestamp {
        self.last_message.as_ref().map(|m| m.at).unwrap_or_default()
    }
}

/// The line the inbox shows under a chat's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagePreview {
    /// The sender's display name, or their ID when they have none.
    pub sender: String,
    /// Whether the signed-in user sent it, so the interface can say "You".
    pub own: bool,
    pub body: MessageBody,
    pub at: Timestamp,
}

/// Identifies one message within its chat.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

/// One thing sent in a chat. Milestone 2 carries text; media comes in milestone 6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub sender: String,
    /// Whether the signed-in user sent it.
    pub own: bool,
    pub at: Timestamp,
    pub body: MessageBody,
    pub state: SendState,
    /// The reactions to it, in the order they were first given.
    pub reactions: Vec<Reaction>,
    /// The message it replies to, if it is a reply.
    pub reply_to: Option<Quote>,
    /// Whether it was edited after it was sent.
    pub edited: bool,
    /// What the user may do with it, as the adapter worked out (a message still on its way out has no ID on the
    /// server yet, so it cannot be replied to or reacted to).
    pub can: Abilities,
    /// Its ID on the server, which stays the same from one run to the next; `None` while it is still being sent.
    pub event_id: Option<String>,
}

impl Message {
    /// A message with no reactions, reply or edits, that nothing can be done with yet. For adapters and tests.
    pub fn new(id: MessageId, sender: impl Into<String>, own: bool, at: Timestamp, body: MessageBody, state: SendState) -> Self {
        Self {
            id,
            sender: sender.into(),
            own,
            at,
            body,
            state,
            reactions: Vec::new(),
            reply_to: None,
            edited: false,
            can: Abilities::default(),
            event_id: None,
        }
    }

    /// The reaction with this key the user gave, if they gave it.
    pub fn my_reaction(&self, key: &str) -> bool {
        self.reactions.iter().any(|r| r.key == key && r.mine)
    }
}

/// More about one message, for its details sheet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageDetails {
    /// When it was sent, as the server (or, for a bridged message, the network) dated it.
    pub sent: Timestamp,
    /// Whether it was end-to-end encrypted on the Matrix side.
    pub encrypted: bool,
    /// Who has read it, by name, and when, where read receipts reach the server.
    pub read_by: Vec<(String, Option<Timestamp>)>,
    /// Its versions, oldest first, when it was edited; empty otherwise.
    pub versions: Vec<(Option<Timestamp>, MessageBody)>,
}

/// What the user may do with one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Abilities {
    pub reply: bool,
    pub react: bool,
    /// Only the user's own text messages.
    pub edit: bool,
    /// Only the user's own messages (deleting other people's needs moderator rights, which PingCraft does not offer).
    pub delete: bool,
}

/// One reaction (an emoji, or any short text a network allows) and who gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    /// What was reacted with, as sent: an emoji, possibly with a variation selector.
    pub key: String,
    /// Who reacted, by display name (or ID when they have none), in the order they did.
    pub by: Vec<String>,
    /// Whether the user is one of them.
    pub mine: bool,
}

/// The message a reply answers, as shown above the reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// The original, when it is among the messages loaded; `None` when it is older or could not be found.
    pub message: Option<MessageId>,
    /// Its sender; empty while it is being fetched.
    pub sender: String,
    pub body: MessageBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    Text(String),
    /// Encrypted, and the key to it has not arrived (yet). Never shown as an error the user caused.
    WaitingForKey,
    /// Deleted by its sender (or a moderator): only the fact that there was a message is left.
    Deleted,
    /// Being fetched from the server (a quoted message that is not loaded yet).
    Loading,
    /// A kind of message this version does not show yet; the text says what it is.
    Unsupported(String),
}

/// Where one of the user's own messages is on its way out. Received messages are always `Sent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendState {
    Sending,
    Sent,
    /// Not sent; the user can try again. The text is the reason in plain words.
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::{Chat, ChatId, MessageBody, MessagePreview, Network, Timestamp};

    /// A chat's settings win over its network's, which win over the default; a saved colour is only ever one of the
    /// network colours.
    #[test]
    fn a_chats_alerts_win_over_its_networks() {
        use super::{Accent, Alerts};
        let network = Alerts { sound: Some(false), accent: Some(Accent::Signal) };
        let chat = Alerts { sound: None, accent: Some(Accent::WhatsApp) };
        assert_eq!(chat.over(network), Alerts { sound: Some(false), accent: Some(Accent::WhatsApp) });
        assert!(!chat.over(network).sounds());
        assert!(Alerts::default().over(Alerts::default()).sounds(), "sound is on unless turned off");
        for accent in Accent::ALL {
            assert_eq!(Accent::from_id(accent.id()), Some(accent));
        }
        assert_eq!(Accent::from_id("#ff0000"), None);
        assert_eq!(Accent::from_id(""), None);
    }

    #[test]
    fn network_ids_are_case_insensitive_and_empty_means_matrix() {
        assert_eq!(Network::from_id("Telegram"), Network::from_id("telegram"));
        assert!(Network::matrix().is_matrix());
        assert!(Network::from_id("  ").is_matrix());
        assert!(!Network::from_id("telegram").is_matrix());
    }

    /// Each network's app's way with symbols that are text by default (PLAN.md, "Messages and chats" 1).
    #[test]
    fn emoji_presentation_follows_each_networks_app() {
        use super::EmojiPresentation::{EmojiFirst, Unicode};
        assert_eq!(Network::from_id("Telegram").emoji_presentation(), EmojiFirst { arrows: true });
        assert_eq!(Network::from_id("signal").emoji_presentation(), EmojiFirst { arrows: false });
        assert_eq!(Network::from_id("whatsapp").emoji_presentation(), EmojiFirst { arrows: false });
        assert_eq!(Network::from_id("discord").emoji_presentation(), Unicode, "not looked at yet");
        assert_eq!(Network::matrix().emoji_presentation(), Unicode);
        assert_eq!(Network::from_id("somewhere-new").emoji_presentation(), Unicode);
    }

    #[test]
    fn risk_levels_follow_the_network_adrs() {
        use super::RiskLevel;
        assert_eq!(Network::from_id("Telegram").risk_level(), Some(RiskLevel::Low));
        assert_eq!(Network::from_id("signal").risk_level(), Some(RiskLevel::Medium));
        assert_eq!(Network::from_id("whatsapp").risk_level(), Some(RiskLevel::MediumHigh));
        assert_eq!(Network::matrix().risk_level(), None);
        assert_eq!(Network::from_id("somewhere-new").risk_level(), None);
        assert!(RiskLevel::Low < RiskLevel::MediumHigh);
    }

    #[test]
    fn a_chat_without_messages_has_the_earliest_activity() {
        let mut chat = Chat {
            id: ChatId("!a".into()),
            account: None,
            network: Network::matrix(),
            name: "A".into(),
            last_message: None,
            unread: 0,
            encrypted: true,
            pinned: false,
            muted: None,
            marked_unread: false,
            archived: None,
            alerts: crate::Alerts::default(),
            draft: None,
            draft_for: None,
        };
        assert_eq!(chat.last_activity(), Timestamp(0));
        chat.last_message = Some(MessagePreview { sender: "Ana".into(), own: false, body: MessageBody::Text("hi".into()), at: Timestamp(5) });
        assert_eq!(chat.last_activity(), Timestamp(5));
    }
}

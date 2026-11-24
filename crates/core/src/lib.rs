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
}

impl Chat {
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

/// One thing sent in a chat. Milestone 2 carries text; media and reactions come in milestone 6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub sender: String,
    /// Whether the signed-in user sent it.
    pub own: bool,
    pub at: Timestamp,
    pub body: MessageBody,
    pub state: SendState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    Text(String),
    /// Encrypted, and the key to it has not arrived (yet). Never shown as an error the user caused.
    WaitingForKey,
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

    #[test]
    fn network_ids_are_case_insensitive_and_empty_means_matrix() {
        assert_eq!(Network::from_id("Telegram"), Network::from_id("telegram"));
        assert!(Network::matrix().is_matrix());
        assert!(Network::from_id("  ").is_matrix());
        assert!(!Network::from_id("telegram").is_matrix());
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
        };
        assert_eq!(chat.last_activity(), Timestamp(0));
        chat.last_message = Some(MessagePreview { sender: "Ana".into(), own: false, body: MessageBody::Text("hi".into()), at: Timestamp(5) });
        assert_eq!(chat.last_activity(), Timestamp(5));
    }
}

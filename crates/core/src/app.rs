//! What the interface asks for and what it is told: the contract between `pingcraft-ui-egui` and whatever does
//! the Matrix work (on the desktop, `pingcraft_matrix::engine`). The interface only sends [`Command`]s and applies
//! [`Event`]s, so it can be tested against a fake [`Backend`] without a server.

use zeroize::Zeroizing;

use crate::bridge::{Answers, BridgeError, BridgedNetwork, LoginStep, StepRef};
use crate::encryption::{RecoveryKey, RecoveryStatus, Verification};
use crate::{AccountId, Alerts, ChatId, ConnectionState, DraftFor, InboxChange, Message, MessageDetails, MessageId, Mute, Network, Quote};

/// Something the user asked for. `Debug` never shows a password, passphrase or message text.
pub enum Command {
    SignIn {
        server: String,
        username: String,
        password: Zeroizing<String>,
    },
    /// The unlock passphrase, where there is no keychain. Opens the vault, or makes it on first use.
    Unlock {
        passphrase: Zeroizing<String>,
    },
    OpenChat(ChatId),
    CloseChat,
    SendText {
        chat: ChatId,
        text: String,
    },
    Retry {
        chat: ChatId,
        message: MessageId,
    },
    /// Sends a text message that replies to another.
    Reply {
        chat: ChatId,
        to: MessageId,
        text: String,
    },
    /// Adds the user's reaction with this key to a message, or takes it back if they had given it.
    React {
        chat: ChatId,
        message: MessageId,
        key: String,
    },
    /// Replaces the text of one of the user's own messages.
    Edit {
        chat: ChatId,
        message: MessageId,
        text: String,
    },
    /// Deletes one of the user's own messages for everyone in the chat.
    Delete {
        chat: ChatId,
        message: MessageId,
    },
    /// Asks for a message's details: [`Event::Details`] answers.
    Details {
        chat: ChatId,
        message: MessageId,
    },
    /// Sends copies of messages from the open chat to another chat, as new messages from the user.
    Forward {
        from: ChatId,
        messages: Vec<MessageId>,
        to: ChatId,
    },
    /// Pins a message in its chat (`pin`), or unpins it. Pins are the chat's, so everyone in it sees them.
    PinMessage {
        chat: ChatId,
        message: MessageId,
        pin: bool,
    },
    /// Pins a chat to the top of the inbox, or unpins it.
    PinChat {
        chat: ChatId,
        pin: bool,
    },
    /// Mutes a chat for a while (`Some`), or turns its notifications back on (`None`).
    Mute {
        chat: ChatId,
        mute: Option<Mute>,
    },
    /// Marks a chat as unread. Opening it marks it read again.
    MarkUnread(ChatId),
    /// Moves a chat out of the inbox into the archive (`archive`), or back. An archived chat comes back by itself
    /// when a new message arrives, unless it is muted.
    Archive {
        chat: ChatId,
        archive: bool,
    },
    /// Sets a chat's own notification sound and accent colour (what is unset follows its network). Kept with the
    /// account, in PingCraft's own room account data.
    ChatAlerts {
        chat: ChatId,
        alerts: Alerts,
    },
    /// Sets a network's notification sound and accent colour, for its chats that do not set their own. Kept with the
    /// account, in PingCraft's own global account data.
    NetworkAlerts {
        network: Network,
        alerts: Alerts,
    },
    /// Keeps what the user typed in a chat and has not sent, with the message it replies to or edits (`draft_for`),
    /// or (empty) forgets it.
    SaveDraft {
        chat: ChatId,
        text: String,
        draft_for: Option<DraftFor>,
    },
    /// Tells the chat whether the user is typing.
    Typing {
        chat: ChatId,
        typing: bool,
    },
    LoadOlder(ChatId),
    /// The user used this emoji (sent it, reacted with it, or picked it). It joins the account's recently used emoji
    /// on the server, which the user's other devices share; uses are sent together a few seconds after the last.
    EmojiUsed(String),
    /// The user chose this skin tone for emoji (by its saved name, such as `medium`). It is kept with the account,
    /// in PingCraft's own account data, so the user's other devices use it too.
    SaveSkinTone(String),
    /// The user saved the recovery key they were shown.
    RecoveryKeySaved,
    /// The user typed the account's recovery key on this device.
    Recover(RecoveryKey),
    /// Ask the user's other devices to verify this one.
    VerifyThisDevice,
    AcceptVerification,
    SymbolsMatch,
    SymbolsDiffer,
    CancelVerification,
    /// Ask each network's bridge how to log in and which accounts the user has there.
    LoadNetworks,
    /// Start connecting an account on a network, with one of its login flows. One login at a time: starting one
    /// ends the one before.
    StartLogin {
        network: Network,
        flow: String,
    },
    /// What the user typed for the login's current question.
    AnswerLogin {
        step: StepRef,
        answers: Answers,
    },
    /// End the login in progress.
    CancelLogin(StepRef),
    /// Log an account out of its network and remove it.
    RemoveAccount {
        network: Network,
        account: AccountId,
    },
    /// Turn on end-to-end encryption in a chat. It cannot be turned off again.
    EnableEncryption(ChatId),
    /// Reconnect to the server now and ask the bridges again, as the header's REFRESH button does.
    Refresh,
    SignOut,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignIn { server, username, .. } => {
                f.debug_struct("SignIn").field("server", server).field("username", username).finish_non_exhaustive()
            }
            Self::Unlock { .. } => f.write_str("Unlock { .. }"),
            Self::OpenChat(c) => f.debug_tuple("OpenChat").field(c).finish(),
            Self::CloseChat => f.write_str("CloseChat"),
            Self::SendText { chat, .. } => f.debug_struct("SendText").field("chat", chat).finish_non_exhaustive(),
            Self::Retry { chat, message } => f.debug_struct("Retry").field("chat", chat).field("message", message).finish(),
            Self::Reply { chat, to, .. } => f.debug_struct("Reply").field("chat", chat).field("to", to).finish_non_exhaustive(),
            // The key is what the user reacted with: as private as the message text.
            Self::React { chat, message, .. } => f.debug_struct("React").field("chat", chat).field("message", message).finish_non_exhaustive(),
            Self::Edit { chat, message, .. } => f.debug_struct("Edit").field("chat", chat).field("message", message).finish_non_exhaustive(),
            Self::Delete { chat, message } => f.debug_struct("Delete").field("chat", chat).field("message", message).finish(),
            Self::Details { chat, message } => f.debug_struct("Details").field("chat", chat).field("message", message).finish(),
            Self::Forward { from, messages, to } => {
                f.debug_struct("Forward").field("from", from).field("messages", messages).field("to", to).finish()
            }
            Self::PinMessage { chat, message, pin } => {
                f.debug_struct("PinMessage").field("chat", chat).field("message", message).field("pin", pin).finish()
            }
            Self::PinChat { chat, pin } => f.debug_struct("PinChat").field("chat", chat).field("pin", pin).finish(),
            Self::Mute { chat, mute } => f.debug_struct("Mute").field("chat", chat).field("mute", mute).finish(),
            Self::MarkUnread(c) => f.debug_tuple("MarkUnread").field(c).finish(),
            Self::ChatAlerts { chat, alerts } => f.debug_struct("ChatAlerts").field("chat", chat).field("alerts", alerts).finish(),
            Self::NetworkAlerts { network, alerts } => f.debug_struct("NetworkAlerts").field("network", network).field("alerts", alerts).finish(),
            Self::Archive { chat, archive } => f.debug_struct("Archive").field("chat", chat).field("archive", archive).finish(),
            Self::SaveDraft { chat, draft_for, .. } => {
                f.debug_struct("SaveDraft").field("chat", chat).field("draft_for", draft_for).finish_non_exhaustive()
            }
            Self::Typing { chat, typing } => f.debug_struct("Typing").field("chat", chat).field("typing", typing).finish(),
            Self::LoadOlder(c) => f.debug_tuple("LoadOlder").field(c).finish(),
            // Which emoji someone uses is as private as what they write.
            Self::EmojiUsed(_) => f.write_str("EmojiUsed(..)"),
            // Says something about the person, so it stays out of logs too.
            Self::SaveSkinTone(_) => f.write_str("SaveSkinTone(..)"),
            Self::RecoveryKeySaved => f.write_str("RecoveryKeySaved"),
            Self::Recover(key) => f.debug_tuple("Recover").field(key).finish(),
            Self::VerifyThisDevice => f.write_str("VerifyThisDevice"),
            Self::AcceptVerification => f.write_str("AcceptVerification"),
            Self::SymbolsMatch => f.write_str("SymbolsMatch"),
            Self::SymbolsDiffer => f.write_str("SymbolsDiffer"),
            Self::CancelVerification => f.write_str("CancelVerification"),
            Self::LoadNetworks => f.write_str("LoadNetworks"),
            Self::StartLogin { network, flow } => f.debug_struct("StartLogin").field("network", network).field("flow", flow).finish(),
            // Answers' own Debug shows the field names only.
            Self::AnswerLogin { step, answers } => f.debug_struct("AnswerLogin").field("step", step).field("answers", answers).finish(),
            Self::CancelLogin(step) => f.debug_tuple("CancelLogin").field(step).finish(),
            Self::RemoveAccount { network, account } => f.debug_struct("RemoveAccount").field("network", network).field("account", account).finish(),
            Self::EnableEncryption(c) => f.debug_tuple("EnableEncryption").field(c).finish(),
            Self::Refresh => f.write_str("Refresh"),
            Self::SignOut => f.write_str("SignOut"),
        }
    }
}

/// Why signing in did not work, for the sign-in sheet to say in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInProblem {
    ServerNotFound,
    /// Nothing answered: no network, or the server is down.
    Unreachable,
    WrongPassword,
    /// The keychain (or vault) could not keep the session.
    Secrets,
    Other(String),
}

/// Something the interface should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Another PingCraft is open with the same data, so this one does nothing: two would overwrite each other's
    /// secrets.
    AlreadyRunning,
    /// Nobody is signed in: show the sign-in sheet.
    SignedOut,
    /// There is no keychain, so PingCraft needs the unlock passphrase. `first` when none has been chosen yet.
    Locked {
        first: bool,
    },
    /// That passphrase did not unlock PingCraft.
    WrongPassphrase,
    SigningIn,
    SignInFailed(SignInProblem),
    /// Signed in as this Matrix ID, shown as typed by the server.
    SignedIn {
        user: String,
    },
    Connection(ConnectionState),
    Inbox(InboxChange),
    /// The open chat's messages, oldest first.
    Messages {
        chat: ChatId,
        messages: Vec<Message>,
    },
    /// A message's details, as asked for with [`Command::Details`].
    Details {
        chat: ChatId,
        message: MessageId,
        details: MessageDetails,
    },
    /// The open chat's pinned messages, oldest pin first.
    Pinned {
        chat: ChatId,
        pinned: Vec<Quote>,
    },
    /// Who is typing in the open chat now, by name (never the user).
    Typing {
        chat: ChatId,
        names: Vec<String>,
    },
    /// The open chat has no older messages.
    ReachedStart(ChatId),
    /// The account's recently used emoji, most recent first: what the server has (shared with the user's other
    /// devices), with this device's uses not sent yet on top. Sent when signed in and whenever it changes.
    RecentEmoji(Vec<String>),
    /// The skin tone for emoji saved with the account (by its saved name), or (`None`) that the account has none:
    /// that is asked of the server itself, never assumed from a sync that has not arrived yet. Sent once signed in
    /// and whenever another device changes it.
    SkinTone(Option<String>),
    /// Each network's notification sound and accent colour, as the account keeps them (networks without settings
    /// are left out). Sent once signed in and whenever they change.
    NetworkAlerts(std::collections::BTreeMap<Network, Alerts>),
    /// The first sign-in made a recovery key: show it until the user has saved it.
    NewRecoveryKey(RecoveryKey),
    Recovery(RecoveryStatus),
    /// That recovery key did not open the key backup.
    WrongRecoveryKey,
    /// A verification started, changed, or (`None`) is over and closed.
    Verification(Option<Verification>),
    /// What a network's bridge said: how to log in and the user's accounts there, or why it could not say.
    Network {
        network: Network,
        found: Result<BridgedNetwork, BridgeError>,
    },
    /// The login in progress is at this step. [`LoginStep::Done`] ends it.
    Login(LoginStep),
    /// The login in progress failed, and has ended.
    LoginFailed {
        network: Network,
        error: BridgeError,
    },
    /// An account was logged out and removed.
    AccountRemoved {
        network: Network,
        account: AccountId,
    },
    /// Removing an account did not work; it is still there.
    RemoveFailed {
        network: Network,
        account: AccountId,
        error: BridgeError,
    },
    /// Something failed that the user should know about, in the adapter's words.
    Problem(String),
}

/// The Matrix side, as the interface sees it.
pub trait Backend {
    fn send(&mut self, command: Command);
    /// Every event since the last call, oldest first. Never blocks.
    fn poll(&mut self) -> Vec<Event>;
}

#[cfg(test)]
mod tests {
    use super::Command;
    use crate::encryption::RecoveryKey;
    use zeroize::Zeroizing;

    #[test]
    fn debug_never_shows_a_password_passphrase_key_or_message() {
        let commands = [
            Command::SignIn { server: "example.org".into(), username: "ana".into(), password: Zeroizing::new("hunter2".into()) },
            Command::Unlock { passphrase: Zeroizing::new("correct horse".into()) },
            Command::Recover(RecoveryKey::new("EsTc 8Jq2")),
            Command::SendText { chat: crate::ChatId("!a".into()), text: "private words".into() },
            Command::Reply { chat: crate::ChatId("!a".into()), to: crate::MessageId("m".into()), text: "private reply".into() },
            Command::Edit { chat: crate::ChatId("!a".into()), message: crate::MessageId("m".into()), text: "private edit".into() },
            Command::React { chat: crate::ChatId("!a".into()), message: crate::MessageId("m".into()), key: "🫣".into() },
            Command::SaveDraft { chat: crate::ChatId("!a".into()), text: "unsent draft".into(), draft_for: None },
            Command::EmojiUsed("🫥".into()),
            Command::SaveSkinTone("medium-dark".into()),
            Command::AnswerLogin {
                step: crate::bridge::StepRef {
                    network: crate::Network::from_id("telegram"),
                    login: "l".into(),
                    step: "s".into(),
                    attempt: "t".into(),
                },
                answers: {
                    let mut a = crate::bridge::Answers::new();
                    a.set("phone", Zeroizing::new("+4512345678".into()));
                    a.set("password", Zeroizing::new("two-step secret".into()));
                    a
                },
            },
        ];
        let shown = format!("{commands:?}");
        for secret in [
            "hunter2",
            "correct horse",
            "EsTc",
            "private words",
            "12345678",
            "two-step secret",
            "private reply",
            "private edit",
            "🫣",
            "unsent draft",
            "🫥",
            "medium-dark",
        ] {
            assert!(!shown.contains(secret), "{shown}");
        }
        assert!(shown.contains("example.org"));
    }
}

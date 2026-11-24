//! What the interface asks for and what it is told: the contract between `pingcraft-ui-egui` and whatever does
//! the Matrix work (on the desktop, `pingcraft_matrix::engine`). The interface only sends [`Command`]s and applies
//! [`Event`]s, so it can be tested against a fake [`Backend`] without a server.

use zeroize::Zeroizing;

use crate::encryption::{RecoveryKey, RecoveryStatus, Verification};
use crate::{ChatId, ConnectionState, InboxChange, Message, MessageId};

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
    LoadOlder(ChatId),
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
            Self::LoadOlder(c) => f.debug_tuple("LoadOlder").field(c).finish(),
            Self::RecoveryKeySaved => f.write_str("RecoveryKeySaved"),
            Self::Recover(key) => f.debug_tuple("Recover").field(key).finish(),
            Self::VerifyThisDevice => f.write_str("VerifyThisDevice"),
            Self::AcceptVerification => f.write_str("AcceptVerification"),
            Self::SymbolsMatch => f.write_str("SymbolsMatch"),
            Self::SymbolsDiffer => f.write_str("SymbolsDiffer"),
            Self::CancelVerification => f.write_str("CancelVerification"),
            Self::SignOut => f.write_str("SignOut"),
        }
    }
}

/// Why signing in did not work, for the sign-in sheet to say in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInProblem {
    ServerNotFound,
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
    /// The open chat has no older messages.
    ReachedStart(ChatId),
    /// The first sign-in made a recovery key: show it until the user has saved it.
    NewRecoveryKey(RecoveryKey),
    Recovery(RecoveryStatus),
    /// That recovery key did not open the key backup.
    WrongRecoveryKey,
    /// A verification started, changed, or (`None`) is over and closed.
    Verification(Option<Verification>),
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
        ];
        let shown = format!("{commands:?}");
        for secret in ["hunter2", "correct horse", "EsTc", "private words"] {
            assert!(!shown.contains(secret), "{shown}");
        }
        assert!(shown.contains("example.org"));
    }
}

//! What the interface knows, built only from the backend's [`Event`]s. No egui here, so it is tested on its own.

use std::collections::BTreeMap;

use pingcraft_core::app::{Event, SignInProblem};
use pingcraft_core::bridge::{BridgeError, BridgedNetwork, LoginStep};
use pingcraft_core::encryption::{RecoveryKey, RecoveryStatus, Verification};
use pingcraft_core::{Account, ChatId, ConnectionState, Filter, Inbox, InboxChange, Message, Network};
use zeroize::Zeroizing;

use crate::i18n::fmt;
use crate::tl;

/// Which main screen is up.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Screen {
    /// Waiting for the backend's first word.
    #[default]
    Starting,
    /// Another PingCraft has this computer's PingCraft data open; this one does nothing.
    AlreadyRunning,
    /// The unlock passphrase (no keychain). `first` when one has to be chosen.
    Locked {
        first: bool,
    },
    SignIn,
    Inbox,
}

/// The sign-in sheet's fields.
#[derive(Default)]
pub struct SignInForm {
    pub server: String,
    pub username: String,
    pub password: Zeroizing<String>,
    pub busy: bool,
    pub problem: Option<SignInProblem>,
}

/// The unlock sheet's fields.
#[derive(Default)]
pub struct UnlockForm {
    pub passphrase: Zeroizing<String>,
    /// Typed twice when choosing one.
    pub again: Zeroizing<String>,
    pub show: bool,
    pub wrong: bool,
    /// The user chose "Sign out" and is being asked to confirm that this computer's data is cleared.
    pub confirm_forget: bool,
}

/// The recovery key sheets: showing a new one, or typing the existing one in.
#[derive(Default)]
pub struct RecoveryForms {
    /// A new key to show until the user has saved it.
    pub new_key: Option<RecoveryKey>,
    pub saved_ticked: bool,
    /// The key was copied, so the Copy button says so.
    pub copied: bool,
    /// The "enter your recovery key" sheet is open.
    pub entering: bool,
    pub typed: Zeroizing<String>,
    pub wrong: bool,
}

/// The tabs in the header, once signed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Inbox,
    Accounts,
}

/// What a network's bridge said, for the ACCOUNTS tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEntry {
    Loading,
    Ready(BridgedNetwork),
    Failed(BridgeError),
}

/// Connecting one network, from the risk screen to the last login step.
#[derive(Debug, Clone)]
pub enum Connect {
    /// The risk screen; `understood` once the user ticked that they accept it.
    Risk {
        network: Network,
        understood: bool,
    },
    /// Choosing one of the bridge's ways to log in.
    Choose {
        network: Network,
    },
    /// Waiting for the bridge's first step.
    Starting {
        network: Network,
    },
    /// A login step; `typed` holds what the user typed into its fields (wiped when dropped), `sent` once answered.
    Step {
        step: LoginStep,
        typed: BTreeMap<String, Zeroizing<String>>,
        sent: bool,
    },
    Failed {
        network: Network,
        error: BridgeError,
    },
}

impl Connect {
    pub fn network(&self) -> Option<&Network> {
        match self {
            Self::Risk { network, .. } | Self::Choose { network } | Self::Starting { network } | Self::Failed { network, .. } => Some(network),
            Self::Step { step, .. } => step.step().map(|s| &s.network),
        }
    }
}

/// The ACCOUNTS tab's state.
#[derive(Debug, Clone, Default)]
pub struct AccountsState {
    pub networks: BTreeMap<Network, NetworkEntry>,
    pub connect: Option<Connect>,
    /// The account the user asked to remove, while they confirm it.
    pub removing: Option<Account>,
    /// The removal was confirmed and sent.
    pub remove_sent: bool,
    /// A line about what just happened (connected, removed, or why not).
    pub notice: Option<String>,
}

impl AccountsState {
    /// Every connected account, in the order of the networks.
    pub fn count(&self) -> usize {
        self.networks
            .values()
            .map(|n| match n {
                NetworkEntry::Ready(n) => n.accounts.len(),
                _ => 0,
            })
            .sum()
    }
}

/// The open chat.
pub struct OpenChat {
    pub id: ChatId,
    pub messages: Vec<Message>,
    pub at_start: bool,
    pub draft: String,
}

#[derive(Default)]
pub struct State {
    pub screen: Screen,
    /// The header's tab, once signed in.
    pub tab: Tab,
    pub accounts: AccountsState,
    pub user: Option<String>,
    pub connection: ConnectionState,
    pub inbox: Inbox,
    /// Whether the inbox has heard from the server yet (an empty inbox before that is not "no chats").
    pub inbox_loaded: bool,
    pub filter: Filter,
    pub search: String,
    pub open: Option<OpenChat>,
    pub sign_in: SignInForm,
    pub unlock: UnlockForm,
    pub recovery: RecoveryStatus,
    pub recovery_forms: RecoveryForms,
    pub verification: Option<Verification>,
    /// The last problem worth telling the user about, until they close it.
    pub problem: Option<String>,
}

impl State {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::AlreadyRunning => *self = Self { screen: Screen::AlreadyRunning, ..Self::default() },
            Event::SignedOut => {
                let server = std::mem::take(&mut self.sign_in.server);
                *self = Self { screen: Screen::SignIn, ..Self::default() };
                self.sign_in.server = server;
            }
            Event::Locked { first } => {
                self.screen = Screen::Locked { first };
                self.unlock = UnlockForm::default();
            }
            Event::WrongPassphrase => {
                self.unlock.wrong = true;
                self.unlock.passphrase = Zeroizing::default();
                self.unlock.again = Zeroizing::default();
            }
            Event::SigningIn => {
                self.sign_in.busy = true;
                self.sign_in.problem = None;
            }
            Event::SignInFailed(problem) => {
                self.sign_in.busy = false;
                self.sign_in.problem = Some(problem);
                self.sign_in.password = Zeroizing::default();
            }
            Event::SignedIn { user } => {
                self.user = Some(user);
                self.screen = Screen::Inbox;
                // The password is not kept once it has done its job.
                self.sign_in = SignInForm { server: std::mem::take(&mut self.sign_in.server), ..SignInForm::default() };
                self.unlock = UnlockForm::default();
            }
            Event::Connection(state) => self.connection = state,
            Event::Inbox(change) => {
                if matches!(change, InboxChange::Reset(_)) {
                    self.inbox_loaded = true;
                }
                self.inbox.apply(change);
            }
            Event::Messages { chat, messages } => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat) {
                    open.messages = messages;
                }
            }
            Event::ReachedStart(chat) => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat) {
                    open.at_start = true;
                }
            }
            Event::NewRecoveryKey(key) => {
                self.recovery_forms.new_key = Some(key);
                self.recovery_forms.saved_ticked = false;
                self.recovery_forms.copied = false;
            }
            Event::Recovery(status) => {
                self.recovery = status;
                if status == RecoveryStatus::On {
                    self.recovery_forms.entering = false;
                    self.recovery_forms.typed = Zeroizing::default();
                }
            }
            Event::WrongRecoveryKey => self.recovery_forms.wrong = true,
            Event::Verification(v) => self.verification = v,
            Event::Network { network, found } => {
                let entry = match found {
                    Ok(n) => NetworkEntry::Ready(n),
                    Err(e) => NetworkEntry::Failed(e),
                };
                self.accounts.networks.insert(network, entry);
            }
            Event::Login(step) => self.login_step(step),
            Event::LoginFailed { network, error } => {
                // Only the login on screen; one the user cancelled has nothing to show.
                if self.accounts.connect.as_ref().and_then(Connect::network) == Some(&network) {
                    self.accounts.connect = Some(Connect::Failed { network, error });
                }
            }
            Event::AccountRemoved { network, account } => {
                self.accounts.removing = None;
                self.accounts.remove_sent = false;
                if let Some(NetworkEntry::Ready(n)) = self.accounts.networks.get_mut(&network) {
                    n.accounts.retain(|a| a.id != account);
                }
                self.accounts.notice = Some(tl!("The account was removed.").to_owned());
            }
            Event::RemoveFailed { error, .. } => {
                self.accounts.removing = None;
                self.accounts.remove_sent = false;
                self.accounts.notice = Some(fmt(tl!("The account was not removed: {why}"), &[("why", &crate::accounts::failure(&error))]));
            }
            Event::Problem(p) => {
                self.sign_in.busy = false;
                self.problem = Some(p);
            }
        }
    }

    /// The next step of the login on screen. A step for a login that is not on screen (cancelled) is dropped.
    fn login_step(&mut self, step: LoginStep) {
        let on_screen = self.accounts.connect.as_ref().and_then(Connect::network).cloned();
        match step {
            LoginStep::Done { network, .. } => {
                if on_screen.as_ref() == Some(&network) {
                    self.accounts.connect = None;
                    let name = crate::widgets::network_name(&network);
                    self.accounts.notice =
                        Some(fmt(tl!("{network} is connected. Its chats show up in your inbox as messages arrive."), &[("network", &name)]));
                }
            }
            step => {
                if step.step().map(|s| &s.network) == on_screen.as_ref() {
                    self.accounts.connect = Some(Connect::Step { step, typed: BTreeMap::new(), sent: false });
                }
            }
        }
    }

    /// Opens a chat (the caller tells the backend).
    pub fn open_chat(&mut self, id: ChatId) {
        if self.open.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        self.open = Some(OpenChat { id, messages: Vec::new(), at_start: false, draft: String::new() });
    }
}

#[cfg(test)]
mod tests {
    use super::{Screen, State};
    use pingcraft_core::app::{Event, SignInProblem};
    use pingcraft_core::{Chat, ChatId, InboxChange, Network};

    fn chat(id: &str) -> Chat {
        Chat { id: ChatId(id.into()), account: None, network: Network::matrix(), name: id.into(), last_message: None, unread: 0, encrypted: true }
    }

    #[test]
    fn signing_in_forgets_the_password_and_keeps_the_server() {
        let mut s = State::default();
        s.apply(Event::SignedOut);
        assert_eq!(s.screen, Screen::SignIn);
        s.sign_in.server = "example.org".into();
        s.sign_in.password = "secret".to_owned().into();
        s.apply(Event::SignInFailed(SignInProblem::WrongPassword));
        assert!(s.sign_in.password.is_empty(), "a refused password is cleared");
        s.sign_in.password = "right".to_owned().into();
        s.apply(Event::SignedIn { user: "@ana:example.org".into() });
        assert_eq!(s.screen, Screen::Inbox);
        assert!(s.sign_in.password.is_empty());
        s.apply(Event::SignedOut);
        assert_eq!(s.sign_in.server, "example.org");
        assert!(s.user.is_none());
    }

    #[test]
    fn messages_only_go_to_the_open_chat() {
        let mut s = State::default();
        s.apply(Event::Inbox(InboxChange::Reset(vec![chat("a"), chat("b")])));
        assert!(s.inbox_loaded);
        s.open_chat(ChatId("a".into()));
        s.apply(Event::Messages { chat: ChatId("b".into()), messages: vec![] });
        s.apply(Event::ReachedStart(ChatId("b".into())));
        assert!(!s.open.as_ref().unwrap().at_start);
        s.apply(Event::ReachedStart(ChatId("a".into())));
        assert!(s.open.as_ref().unwrap().at_start);
    }
}

//! What the interface knows, built only from the backend's [`Event`]s. No egui here, so it is tested on its own.

use std::collections::BTreeMap;

use pingcraft_core::app::{Command, Event, SignInProblem};
use pingcraft_core::bridge::{BridgeError, BridgedNetwork, LoginStep};
use pingcraft_core::encryption::{RecoveryKey, RecoveryStatus, Verification};
use pingcraft_core::notify::Notification;
use pingcraft_core::{
    Account, Alerts, ChatId, ConnectionState, DraftFor, Filter, Inbox, InboxChange, Message, MessageBody, MessageDetails, MessageId, Network, Quote,
};
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
    /// The "turn on encryption" sheet is open.
    pub encrypt_sheet: bool,
    /// The message the composer replies to.
    pub replying: Option<MessageId>,
    /// The user's own message the composer is editing, and the draft it set aside.
    pub editing: Option<(MessageId, String)>,
    /// The messages the user asked to delete, while they confirm it.
    pub deleting: Vec<MessageId>,
    /// A quoted message was clicked: scroll to it.
    pub jump_to: Option<MessageId>,
    /// The details sheet: which message, and its details once they came.
    pub details: Option<(MessageId, Option<MessageDetails>)>,
    /// Selecting messages: the ones ticked. `None` when not selecting.
    pub selected: Option<Vec<MessageId>>,
    /// The forward sheet.
    pub forwarding: Option<Forwarding>,
    /// The chat's pinned messages, oldest pin first.
    pub pinned: Vec<Quote>,
    /// Which pin the pinned bar shows, counting from the newest.
    pub pin_shown: usize,
    /// Who is typing, by name.
    pub typing: Vec<String>,
    /// When (egui's clock, seconds) the user was last said to be typing; `None` when they are not.
    pub typing_told: Option<f64>,
    /// Whether the timeline was at its newest message in the last frame.
    pub at_bottom: bool,
    /// JUMP TO LATEST was clicked: scroll to the newest message.
    pub to_bottom: bool,
    /// The message a reaction is being chosen for in the emoji picker.
    pub reacting: Option<MessageId>,
    /// The reply or edit kept with the draft, waiting for the chat's messages to find its message.
    pub restoring: Option<DraftFor>,
}

/// Forwarding messages from the open chat: which ones, and where to.
#[derive(Debug, Clone, Default)]
pub struct Forwarding {
    pub messages: Vec<MessageId>,
    pub to: Option<ChatId>,
    pub search: String,
}

impl OpenChat {
    pub fn new(id: ChatId) -> Self {
        Self {
            id,
            messages: Vec::new(),
            at_start: false,
            draft: String::new(),
            encrypt_sheet: false,
            replying: None,
            editing: None,
            deleting: Vec::new(),
            jump_to: None,
            details: None,
            selected: None,
            forwarding: None,
            pinned: Vec::new(),
            pin_shown: 0,
            typing: Vec::new(),
            typing_told: None,
            at_bottom: true,
            to_bottom: false,
            reacting: None,
            restoring: None,
        }
    }

    /// Ticks or unticks a message while selecting.
    pub fn toggle(&mut self, id: MessageId) {
        let Some(selected) = self.selected.as_mut() else { return };
        match selected.iter().position(|m| *m == id) {
            Some(i) => {
                selected.remove(i);
            }
            None => selected.push(id),
        }
    }

    /// The selected messages, in the chat's order.
    pub fn selection(&self) -> Vec<&Message> {
        let selected = self.selected.as_deref().unwrap_or_default();
        self.messages.iter().filter(|m| selected.contains(&m.id)).collect()
    }

    pub fn message(&self, id: &MessageId) -> Option<&Message> {
        self.messages.iter().find(|m| &m.id == id)
    }

    /// Starts editing one of the user's messages: its text goes into the composer, and the draft waits.
    pub fn edit(&mut self, id: MessageId) {
        let Some(MessageBody::Text(text)) = self.message(&id).map(|m| m.body.clone()) else { return };
        self.replying = None;
        let before = match self.editing.take() {
            Some((_, before)) => before,
            None => std::mem::take(&mut self.draft),
        };
        self.draft = text;
        self.editing = Some((id, before));
    }

    /// Stops editing and gives the draft back.
    pub fn stop_editing(&mut self) {
        if let Some((_, before)) = self.editing.take() {
            self.draft = before;
        }
    }

    /// Picks up the reply or edit kept with the draft once the messages have come: the reply bar, or the edit, over
    /// the composer. When its message is not among them, it is dropped and the text stays as a new message's draft.
    fn restore(&mut self) {
        if self.messages.is_empty() {
            return;
        }
        let Some(draft_for) = self.restoring.take() else { return };
        let find = |event: &str| self.messages.iter().find(|m| m.event_id.as_deref() == Some(event)).map(|m| m.id.clone());
        match draft_for {
            DraftFor::Reply(event) => self.replying = find(&event),
            // The draft is the edited text; nothing was set aside for it (the SDK keeps one draft per chat).
            DraftFor::Edit(event) => self.editing = find(&event).map(|id| (id, String::new())),
        }
    }

    /// The draft to keep when leaving the chat, and what it is for. An edit is kept when it changed something;
    /// otherwise the draft it set aside is kept, as a new message's.
    pub fn draft_to_keep(&self) -> (String, Option<DraftFor>) {
        let event = |id: &MessageId| self.message(id).and_then(|m| m.event_id.clone());
        let text = self.draft.trim().to_owned();
        if let Some((id, before)) = &self.editing {
            let original = match self.message(id).map(|m| &m.body) {
                Some(MessageBody::Text(t)) => t.trim(),
                _ => "",
            };
            return match event(id) {
                Some(event) if text != original && !text.is_empty() => (text, Some(DraftFor::Edit(event))),
                _ => (before.trim().to_owned(), None),
            };
        }
        if text.is_empty() {
            return (text, None);
        }
        match (&self.replying, &self.restoring) {
            (Some(id), _) => (text, event(id).map(DraftFor::Reply)),
            (None, restoring) => (text, restoring.clone()),
        }
    }

    /// Takes in a new list of messages; a reply, edit or deletion of a message that is gone is dropped.
    fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.restore();
        let gone = |id: &Option<MessageId>, list: &[Message]| id.as_ref().is_some_and(|id| !list.iter().any(|m| &m.id == id));
        if gone(&self.replying, &self.messages) {
            self.replying = None;
        }
        let list = &self.messages;
        self.deleting.retain(|id| list.iter().any(|m| &m.id == id));
        if let Some(selected) = self.selected.as_mut() {
            selected.retain(|id| list.iter().any(|m| &m.id == id));
        }
        if gone(&self.details.as_ref().map(|(id, _)| id.clone()), &self.messages) {
            self.details = None;
        }
        if gone(&self.editing.as_ref().map(|(id, _)| id.clone()), &self.messages) {
            self.stop_editing();
        }
    }
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
    /// Whether the window has focus, as of this frame.
    pub focused: bool,
    /// Desktop notifications to show, taken by the app each frame.
    pub notifications: Vec<Notification>,
    /// Each network's notification sound and accent colour, as the account keeps them.
    pub network_alerts: BTreeMap<Network, Alerts>,
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
                // The open chat's messages are told about as they arrive (below).
                if let InboxChange::Upsert(chat) = &change
                    && self.inbox_loaded
                    && self.open.as_ref().is_none_or(|o| o.id != chat.id)
                    && let Some(mut n) = crate::notify::for_inbox_change(self.inbox.get(&chat.id), chat)
                {
                    n.sound = self.alerts(chat).sounds();
                    self.notifications.push(n);
                }
                self.inbox.apply(change);
            }
            Event::Messages { chat, messages } => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat) {
                    // The chat is in sight only on the INBOX tab of a window with focus.
                    let seen = self.focused && self.tab == Tab::Inbox;
                    if let Some(mut n) = crate::notify::for_open_chat(&open.messages, &messages, self.inbox.get(&chat), seen) {
                        n.sound = self.inbox.get(&chat).is_none_or(|c| Self::alerts_with(&self.network_alerts, c).sounds());
                        self.notifications.push(n);
                    }
                    open.set_messages(messages);
                }
            }
            Event::Typing { chat, names } => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat) {
                    open.typing = names;
                }
            }
            Event::Pinned { chat, pinned } => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat) {
                    open.pinned = pinned;
                    if open.pin_shown >= open.pinned.len() {
                        open.pin_shown = 0;
                    }
                }
            }
            Event::Details { chat, message, details } => {
                if let Some(open) = self.open.as_mut().filter(|o| o.id == chat)
                    && let Some((id, shown)) = open.details.as_mut()
                    && *id == message
                {
                    *shown = Some(details);
                }
            }
            // Kept with the picker (`PingCraftApp::ui`), which draws them.
            Event::RecentEmoji(_) | Event::SkinTone(_) => {}
            Event::NetworkAlerts(alerts) => self.network_alerts = alerts,
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

    /// A chat's notification sound and accent colour: its own settings over its network's.
    pub fn alerts(&self, chat: &pingcraft_core::Chat) -> Alerts {
        Self::alerts_with(&self.network_alerts, chat)
    }

    fn alerts_with(networks: &BTreeMap<Network, Alerts>, chat: &pingcraft_core::Chat) -> Alerts {
        chat.alerts.over(networks.get(&chat.network).copied().unwrap_or_default())
    }

    /// Opens a chat (the caller tells the backend), with its draft in the composer.
    pub fn open_chat(&mut self, id: ChatId) {
        if self.open.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        let mut open = OpenChat::new(id);
        if let Some(chat) = self.inbox.get(&open.id) {
            open.draft = chat.draft.clone().unwrap_or_default();
            open.restoring = chat.draft_for.clone();
        }
        self.open = Some(open);
    }

    /// What to tell the backend when the user leaves the open chat: its draft, when it changed, and that they
    /// stopped typing.
    pub fn leaving(&self) -> Vec<Command> {
        let Some(open) = &self.open else { return Vec::new() };
        let mut out = Vec::new();
        if open.typing_told.is_some() {
            out.push(Command::Typing { chat: open.id.clone(), typing: false });
        }
        let (text, draft_for) = open.draft_to_keep();
        let kept = self.inbox.get(&open.id).map(|c| (c.draft.as_deref().unwrap_or_default(), c.draft_for.as_ref()));
        if kept != Some((text.as_str(), draft_for.as_ref())) && !(kept.is_none() && text.is_empty()) {
            out.push(Command::SaveDraft { chat: open.id.clone(), text, draft_for });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Screen, State};
    use pingcraft_core::app::{Event, SignInProblem};
    use pingcraft_core::{Chat, ChatId, InboxChange, Network};

    fn chat(id: &str) -> Chat {
        Chat {
            id: ChatId(id.into()),
            account: None,
            network: Network::matrix(),
            name: id.into(),
            last_message: None,
            unread: 0,
            encrypted: true,
            pinned: false,
            muted: None,
            marked_unread: false,
            archived: None,
            alerts: pingcraft_core::Alerts::default(),
            draft: None,
            draft_for: None,
        }
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
    fn editing_sets_the_draft_aside_and_gives_it_back() {
        use pingcraft_core::{Message, MessageBody, MessageId, SendState, Timestamp};
        let mut s = State::default();
        s.open_chat(ChatId("a".into()));
        let mine = Message::new(MessageId("1".into()), "me", true, Timestamp(1), MessageBody::Text("old words".into()), SendState::Sent);
        s.apply(Event::Messages { chat: ChatId("a".into()), messages: vec![mine] });
        let open = s.open.as_mut().unwrap();
        open.draft = "half a thought".into();
        open.edit(MessageId("1".into()));
        assert_eq!(open.draft, "old words");
        open.stop_editing();
        assert_eq!(open.draft, "half a thought");
        open.edit(MessageId("1".into()));
        // The message goes away (deleted elsewhere): the edit ends and the draft comes back.
        s.apply(Event::Messages { chat: ChatId("a".into()), messages: vec![] });
        let open = s.open.as_ref().unwrap();
        assert!(open.editing.is_none());
        assert_eq!(open.draft, "half a thought");
    }

    #[test]
    fn the_open_chat_behind_another_tab_still_notifies() {
        use pingcraft_core::{Message, MessageBody, MessageId, SendState, Timestamp};
        let m = |id: &str| Message::new(MessageId(id.into()), "Lars", false, Timestamp(1), MessageBody::Text("hi".into()), SendState::Sent);
        let mut s = State::default();
        s.apply(Event::Inbox(InboxChange::Reset(vec![chat("a")])));
        s.open_chat(ChatId("a".into()));
        s.focused = true;
        s.apply(Event::Messages { chat: ChatId("a".into()), messages: vec![m("1")] });
        s.apply(Event::Messages { chat: ChatId("a".into()), messages: vec![m("1"), m("2")] });
        assert!(s.notifications.is_empty(), "in sight");
        s.tab = crate::Tab::Accounts;
        s.apply(Event::Messages { chat: ChatId("a".into()), messages: vec![m("1"), m("2"), m("3")] });
        assert_eq!(s.notifications.len(), 1, "on the ACCOUNTS tab the chat is out of sight");
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

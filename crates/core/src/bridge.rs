//! Bridge providers (ADR 0003): connecting a network from inside PingCraft, in PingCraft's own words.
//!
//! A [`BridgeProvider`] is a family of bridges (the first is mautrix, in `pingcraft-matrix`). It says which networks
//! it can connect on the user's server, walks a login through its steps (show a QR code or a code and wait, ask for
//! a phone number or a code, done), lists the user's accounts on each network, and removes one. Nothing here knows
//! how a provider talks to its bridges, so another provider, or PingCraft's own bridges, can take its place without
//! changing a screen.
//!
//! Which network a *chat* is on is not the provider's business: that comes from the bridge-info room state.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use zeroize::Zeroizing;

use crate::{Account, AccountId, Network};

/// A future a provider returns. Boxed, so providers can be swapped behind `dyn BridgeProvider`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One way to log in to a network, in the bridge's words (e.g. "QR code", "Phone number").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginFlow {
    /// The provider's name for it; given back to [`BridgeProvider::start_login`].
    pub id: String,
    pub name: String,
    pub description: String,
}

/// A network a provider can connect on this server: how to log in, and the accounts the user has on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgedNetwork {
    pub network: Network,
    pub flows: Vec<LoginFlow>,
    pub accounts: Vec<Account>,
}

/// Identifies one step of one login in progress. The fields are the provider's own: only the provider that made
/// a step reads them, so nothing else should build one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRef {
    pub network: Network,
    /// The login in progress.
    pub login: String,
    /// The step within it.
    pub step: String,
    /// What makes answering this step safe to retry.
    pub attempt: String,
}

/// What a waiting step shows the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Display {
    /// A QR code with this content, to scan with the network's app.
    Qr(String),
    /// A short code to type into the network's app.
    Code(String),
    /// Emoji to compare.
    Emoji(String),
    /// Nothing to show: just wait (for a confirmation on the phone, say).
    Nothing,
}

/// What a field asks for, so the interface can pick the right keyboard, hide a password and say what is meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    PhoneNumber,
    /// A code the network sent, such as a login or two-factor code.
    Code,
    Password,
    Username,
    Email,
    Token,
    Url,
    Domain,
    /// Anything else, by the provider's name for it.
    Text(String),
}

impl FieldKind {
    /// Whether what is typed must be hidden and wiped.
    pub fn is_secret(&self) -> bool {
        matches!(self, Self::Password | Self::Token)
    }
}

/// One thing to type in a login step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputField {
    /// The provider's name for the field; the key in [`Answers`].
    pub id: String,
    pub kind: FieldKind,
    /// What to call it, in the bridge's words.
    pub name: String,
    pub description: String,
    pub default: Option<String>,
    pub optional: bool,
}

/// Where a login is, and what the user has to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginStep {
    /// Show something and wait: [`BridgeProvider::wait`] returns when it changes (a new QR code, or the next step).
    Show { step: StepRef, instructions: String, display: Display, can_cancel: bool },
    /// Ask the user to type something, then [`BridgeProvider::submit`] it.
    Ask { step: StepRef, instructions: String, fields: Vec<InputField>, can_cancel: bool },
    /// A step PingCraft cannot do yet, such as logging in through a web page; `kind` is the provider's name for
    /// it. The login can only be cancelled.
    Unsupported { step: StepRef, kind: String, instructions: String },
    /// Logged in: the network has a new account.
    Done { network: Network, account: AccountId },
}

impl LoginStep {
    /// The step to answer, wait on or cancel; `None` once the login is done.
    pub fn step(&self) -> Option<&StepRef> {
        match self {
            Self::Show { step, .. } | Self::Ask { step, .. } | Self::Unsupported { step, .. } => Some(step),
            Self::Done { .. } => None,
        }
    }
}

/// What the user typed for an [`LoginStep::Ask`], by field ID. The values (phone numbers, codes, passwords) are
/// wiped from memory when dropped, and `Debug` shows only the field IDs.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct Answers(BTreeMap<String, Zeroizing<String>>);

impl Answers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, field: &str, value: Zeroizing<String>) {
        self.0.insert(field.to_owned(), value);
    }

    pub fn get(&self, field: &str) -> Option<&str> {
        self.0.get(field).map(|v| v.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Answers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Answers").field(&self.0.keys().collect::<Vec<_>>()).finish()
    }
}

/// Why a provider could not do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// That network's bridge is not on this server.
    NotAvailable,
    /// The bridge did not accept who PingCraft said the user is.
    NotAuthorized,
    /// The bridge said no, in its own words (a wrong code, too many accounts, ...).
    Refused(String),
    /// The login in progress has ended (it timed out or was cancelled).
    LoginGone,
    /// The bridge could not be reached, or answered something PingCraft does not understand.
    Unreachable(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => f.write_str("that network's bridge is not on this server"),
            Self::NotAuthorized => f.write_str("the bridge did not accept who you are"),
            Self::Refused(why) => write!(f, "the bridge said: {why}"),
            Self::LoginGone => f.write_str("that login has ended"),
            Self::Unreachable(why) => write!(f, "the bridge could not be reached: {why}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// A family of bridges. Every call goes to the user's own server; none of them logs what the user types.
pub trait BridgeProvider: Send + Sync {
    /// The networks this provider is set up for, whether or not their bridges answer.
    fn networks(&self) -> Vec<Network>;

    /// How to log in to a network, and the user's accounts on it. [`BridgeError::NotAvailable`] when its bridge is
    /// not on this server.
    fn network<'a>(&'a self, network: &'a Network) -> BoxFuture<'a, Result<BridgedNetwork, BridgeError>>;

    /// Starts logging in with one of the network's [`LoginFlow`]s, and returns the first step.
    fn start_login<'a>(&'a self, network: &'a Network, flow: &'a str) -> BoxFuture<'a, Result<LoginStep, BridgeError>>;

    /// Answers an [`LoginStep::Ask`], and returns the next step.
    fn submit<'a>(&'a self, step: &'a StepRef, answers: Answers) -> BoxFuture<'a, Result<LoginStep, BridgeError>>;

    /// Waits on a [`LoginStep::Show`] until it changes, and returns what comes next. It can take minutes; drop the
    /// future to stop waiting, and [`BridgeProvider::cancel`] to end the login.
    fn wait<'a>(&'a self, step: &'a StepRef) -> BoxFuture<'a, Result<LoginStep, BridgeError>>;

    /// Ends a login in progress. Ending one that has already ended is not an error.
    fn cancel<'a>(&'a self, step: &'a StepRef) -> BoxFuture<'a, Result<(), BridgeError>>;

    /// Logs an account out of its network and removes it from the bridge.
    fn remove_account<'a>(&'a self, network: &'a Network, account: &'a AccountId) -> BoxFuture<'a, Result<(), BridgeError>>;
}

#[cfg(test)]
mod tests {
    use super::{Answers, BoxFuture, BridgeError, BridgeProvider, BridgedNetwork, FieldKind, LoginStep, StepRef};
    use crate::{AccountId, Network};
    use zeroize::Zeroizing;

    #[test]
    fn answers_never_show_what_was_typed() {
        let mut answers = Answers::new();
        answers.set("phone_number", Zeroizing::new("+4512345678".into()));
        answers.set("password", Zeroizing::new("hunter2".into()));
        let shown = format!("{answers:?}");
        assert!(!shown.contains("12345678") && !shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("phone_number"));
        assert_eq!(answers.get("password"), Some("hunter2"));
    }

    #[test]
    fn secrets_are_the_fields_to_hide() {
        assert!(FieldKind::Password.is_secret() && FieldKind::Token.is_secret());
        assert!(!FieldKind::PhoneNumber.is_secret() && !FieldKind::Code.is_secret());
    }

    #[test]
    fn a_done_login_has_no_step_left() {
        let step = StepRef { network: Network::from_id("signal"), login: "l".into(), step: "s".into(), attempt: "a".into() };
        let unsupported = LoginStep::Unsupported { step: step.clone(), kind: "cookies".into(), instructions: String::new() };
        assert_eq!(unsupported.step(), Some(&step));
        assert_eq!(LoginStep::Done { network: Network::from_id("signal"), account: AccountId("x".into()) }.step(), None);
    }

    /// A provider with no bridges: shows that the trait can be used behind `dyn`, as swapping providers needs.
    struct Nothing;

    impl BridgeProvider for Nothing {
        fn networks(&self) -> Vec<Network> {
            Vec::new()
        }
        fn network<'a>(&'a self, _: &'a Network) -> BoxFuture<'a, Result<BridgedNetwork, BridgeError>> {
            Box::pin(async { Err(BridgeError::NotAvailable) })
        }
        fn start_login<'a>(&'a self, _: &'a Network, _: &'a str) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            Box::pin(async { Err(BridgeError::NotAvailable) })
        }
        fn submit<'a>(&'a self, _: &'a StepRef, _: Answers) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            Box::pin(async { Err(BridgeError::LoginGone) })
        }
        fn wait<'a>(&'a self, _: &'a StepRef) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            Box::pin(async { Err(BridgeError::LoginGone) })
        }
        fn cancel<'a>(&'a self, _: &'a StepRef) -> BoxFuture<'a, Result<(), BridgeError>> {
            Box::pin(async { Ok(()) })
        }
        fn remove_account<'a>(&'a self, _: &'a Network, _: &'a AccountId) -> BoxFuture<'a, Result<(), BridgeError>> {
            Box::pin(async { Err(BridgeError::NotAvailable) })
        }
    }

    #[test]
    fn providers_can_be_swapped_behind_dyn() {
        let provider: Box<dyn BridgeProvider> = Box::new(Nothing);
        assert!(provider.networks().is_empty());
    }
}

//! Connecting networks, for the desktop engine: runs a [`BridgeProvider`]'s calls in the background and turns what
//! they return into [`Event`]s.
//!
//! One login runs at a time. While a step shows something (a QR code), this waits on it in the background, and
//! shows the next step when it comes (a new QR code, a question, done). Starting another login or cancelling stops
//! the wait and ends the login at the bridge. Answers from a login that has since ended are dropped.

use std::sync::Arc;

use matrix_sdk::executor::{AbortOnDrop, spawn};
use pingcraft_core::app::Event;
use pingcraft_core::bridge::{Answers, BridgeError, BridgeProvider, BridgedNetwork, LoginStep, StepRef};
use pingcraft_core::{AccountId, Network};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// What a background call came back with.
pub(crate) enum Done {
    Network(Network, Result<BridgedNetwork, BridgeError>),
    /// A login step, from the login with this number.
    Step(u64, Network, Result<LoginStep, BridgeError>),
    Removed(Network, AccountId, Result<(), BridgeError>),
}

/// The login in progress.
struct Login {
    number: u64,
    /// The step the user is on, once there is one.
    step: Option<StepRef>,
    /// The call in flight for it (starting, answering or waiting); dropping it stops the call.
    task: Option<AbortOnDrop<()>>,
}

pub(crate) struct Connector {
    provider: Arc<dyn BridgeProvider>,
    tx: UnboundedSender<Done>,
    rx: UnboundedReceiver<Done>,
    login: Option<Login>,
    logins: u64,
}

impl Connector {
    pub(crate) fn new(provider: Arc<dyn BridgeProvider>) -> Self {
        let (tx, rx) = unbounded_channel();
        Self { provider, tx, rx, login: None, logins: 0 }
    }

    /// The next result of a background call. Never ends: the connector holds a sender itself.
    pub(crate) async fn next(&mut self) -> Option<Done> {
        self.rx.recv().await
    }

    /// Asks every network's bridge for its login flows and the user's accounts.
    pub(crate) fn load(&self) {
        for network in self.provider.networks() {
            self.network(network);
        }
    }

    fn network(&self, network: Network) {
        let (provider, tx) = (self.provider.clone(), self.tx.clone());
        drop(spawn(async move {
            let found = provider.network(&network).await;
            let _ = tx.send(Done::Network(network, found));
        }));
    }

    pub(crate) fn start(&mut self, network: Network, flow: String) {
        self.end_login();
        self.logins += 1;
        let number = self.logins;
        let (provider, tx, at) = (self.provider.clone(), self.tx.clone(), network);
        let task = AbortOnDrop::new(spawn(async move {
            let step = provider.start_login(&at, &flow).await;
            let _ = tx.send(Done::Step(number, at, step));
        }));
        self.login = Some(Login { number, step: None, task: Some(task) });
    }

    /// Answers the current question. An answer to any other step is dropped.
    pub(crate) fn answer(&mut self, step: StepRef, answers: Answers) {
        let Some(login) = self.login.as_mut().filter(|l| l.step.as_ref() == Some(&step)) else { return };
        let (provider, tx, number) = (self.provider.clone(), self.tx.clone(), login.number);
        login.task = Some(AbortOnDrop::new(spawn(async move {
            let next = provider.submit(&step, answers).await;
            let _ = tx.send(Done::Step(number, step.network.clone(), next));
        })));
    }

    /// Ends the login at `step`, if it is the one in progress.
    pub(crate) fn cancel(&mut self, step: &StepRef) {
        if self.login.as_ref().is_some_and(|l| l.step.as_ref() == Some(step)) {
            self.end_login();
        }
    }

    pub(crate) fn remove(&self, network: Network, account: AccountId) {
        let (provider, tx) = (self.provider.clone(), self.tx.clone());
        drop(spawn(async move {
            let removed = provider.remove_account(&network, &account).await;
            let _ = tx.send(Done::Removed(network, account, removed));
        }));
    }

    /// Stops the login in progress, and ends it at the bridge in the background.
    fn end_login(&mut self) {
        let Some(login) = self.login.take() else { return };
        drop(login.task);
        if let Some(step) = login.step {
            let provider = self.provider.clone();
            drop(spawn(async move {
                let _ = provider.cancel(&step).await;
            }));
        }
    }

    /// What a finished call means for the interface. Keeps the login going: a shown step is waited on.
    pub(crate) fn handle(&mut self, done: Done) -> Vec<Event> {
        match done {
            Done::Network(network, found) => vec![Event::Network { network, found }],
            Done::Removed(network, account, Ok(())) => {
                self.network(network.clone());
                vec![Event::AccountRemoved { network, account }]
            }
            Done::Removed(network, account, Err(error)) => vec![Event::RemoveFailed { network, account, error }],
            Done::Step(number, network, result) => {
                let Some(login) = self.login.as_mut().filter(|l| l.number == number) else { return Vec::new() };
                match result {
                    Err(error) => {
                        self.login = None;
                        vec![Event::LoginFailed { network, error }]
                    }
                    Ok(LoginStep::Done { network, account }) => {
                        self.login = None;
                        self.network(network.clone());
                        vec![Event::Login(LoginStep::Done { network, account })]
                    }
                    Ok(step) => {
                        login.step = step.step().cloned();
                        login.task = None;
                        if let LoginStep::Show { step: at, .. } = &step {
                            let (provider, tx, at) = (self.provider.clone(), self.tx.clone(), at.clone());
                            login.task = Some(AbortOnDrop::new(spawn(async move {
                                let next = provider.wait(&at).await;
                                let _ = tx.send(Done::Step(number, at.network.clone(), next));
                            })));
                        }
                        vec![Event::Login(step)]
                    }
                }
            }
        }
    }
}

impl Drop for Connector {
    fn drop(&mut self) {
        self.end_login();
    }
}

#[cfg(test)]
mod tests {
    //! The connector against a scripted provider: the order of events, waiting on shown steps, and stale answers.
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use pingcraft_core::app::Event;
    use pingcraft_core::bridge::{Answers, BoxFuture, BridgeError, BridgeProvider, BridgedNetwork, Display, LoginStep, StepRef};
    use pingcraft_core::{AccountId, Network};

    use super::Connector;

    /// Answers start, submit and wait from a script, in order, and records cancels.
    #[derive(Default)]
    struct Scripted {
        steps: Mutex<VecDeque<Result<LoginStep, BridgeError>>>,
        cancelled: Mutex<Vec<StepRef>>,
    }

    impl Scripted {
        fn next(&self) -> BoxFuture<'_, Result<LoginStep, BridgeError>> {
            let next = self.steps.lock().unwrap().pop_front();
            Box::pin(async move {
                match next {
                    Some(step) => step,
                    // Nothing scripted: wait forever, like a QR code nobody scans.
                    None => std::future::pending().await,
                }
            })
        }
    }

    impl BridgeProvider for Scripted {
        fn networks(&self) -> Vec<Network> {
            vec![Network::from_id("signal")]
        }
        fn network<'a>(&'a self, network: &'a Network) -> BoxFuture<'a, Result<BridgedNetwork, BridgeError>> {
            Box::pin(async move { Ok(BridgedNetwork { network: network.clone(), flows: vec![], accounts: vec![] }) })
        }
        fn start_login<'a>(&'a self, _: &'a Network, _: &'a str) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            self.next()
        }
        fn submit<'a>(&'a self, _: &'a StepRef, _: Answers) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            self.next()
        }
        fn wait<'a>(&'a self, _: &'a StepRef) -> BoxFuture<'a, Result<LoginStep, BridgeError>> {
            self.next()
        }
        fn cancel<'a>(&'a self, step: &'a StepRef) -> BoxFuture<'a, Result<(), BridgeError>> {
            self.cancelled.lock().unwrap().push(step.clone());
            Box::pin(async { Ok(()) })
        }
        fn remove_account<'a>(&'a self, _: &'a Network, _: &'a AccountId) -> BoxFuture<'a, Result<(), BridgeError>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn step(id: &str) -> StepRef {
        StepRef { network: Network::from_id("signal"), login: "l".into(), step: id.into(), attempt: id.into() }
    }

    fn qr(id: &str, data: &str) -> LoginStep {
        LoginStep::Show { step: step(id), instructions: String::new(), display: Display::Qr(data.into()), can_cancel: false }
    }

    async fn next_events(c: &mut Connector) -> Vec<Event> {
        let done = c.next().await.unwrap();
        c.handle(done)
    }

    #[tokio::test]
    async fn a_shown_code_is_waited_on_until_the_login_is_done() {
        let provider = Arc::new(Scripted::default());
        provider.steps.lock().unwrap().extend([
            Ok(qr("qr", "first")),
            Ok(qr("qr", "refreshed")),
            Ok(LoginStep::Done { network: Network::from_id("signal"), account: AccountId("new".into()) }),
        ]);
        let mut c = Connector::new(provider.clone());
        c.start(Network::from_id("signal"), "qr".into());
        assert_eq!(next_events(&mut c).await, vec![Event::Login(qr("qr", "first"))]);
        assert_eq!(next_events(&mut c).await, vec![Event::Login(qr("qr", "refreshed"))], "waited on by itself");
        assert!(matches!(&next_events(&mut c).await[..], [Event::Login(LoginStep::Done { .. })]));
        // The account list of that network is asked for again.
        assert!(matches!(&next_events(&mut c).await[..], [Event::Network { .. }]));
        assert!(provider.cancelled.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancelling_ends_the_login_at_the_bridge_and_drops_what_comes_after() {
        let provider = Arc::new(Scripted::default());
        provider.steps.lock().unwrap().push_back(Ok(qr("qr", "first")));
        let mut c = Connector::new(provider.clone());
        c.start(Network::from_id("signal"), "qr".into());
        assert_eq!(next_events(&mut c).await, vec![Event::Login(qr("qr", "first"))]);
        c.cancel(&step("qr"));
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(*provider.cancelled.lock().unwrap(), vec![step("qr")]);
        // A late answer from the ended login is dropped.
        assert!(c.handle(super::Done::Step(1, Network::from_id("signal"), Ok(qr("qr", "late")))).is_empty());
        // An answer to a step that is not the current one is ignored.
        c.answer(step("other"), Answers::new());
    }

    #[tokio::test]
    async fn a_failed_step_ends_the_login() {
        let provider = Arc::new(Scripted::default());
        provider.steps.lock().unwrap().push_back(Err(BridgeError::NotAuthorized));
        let mut c = Connector::new(provider);
        c.start(Network::from_id("signal"), "qr".into());
        assert_eq!(next_events(&mut c).await, vec![Event::LoginFailed { network: Network::from_id("signal"), error: BridgeError::NotAuthorized }]);
        assert!(c.login.is_none());
    }
}

//! The desktop's Matrix side, as a [`Backend`] the interface can drive: a thread with its own async runtime that
//! takes [`Command`]s and sends back [`Event`]s, all `pingcraft-core` types.
//!
//! Where secrets go: the OS keychain when it answers, otherwise a [`PassphraseVault`] in the data folder that the
//! user unlocks with a passphrase (ADR 0004). The only plain file is `profile`, the random name of the signed-in
//! profile, which says nothing about the account.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use pingcraft_core::app::{Backend, Command, Event, SignInProblem};
use pingcraft_core::encryption::{RecoveryStatus, Verification};
use pingcraft_core::{ChatId, ConnectionState};
use pingcraft_keychain::{KdfParams, OsKeychain, PassphraseVault, SecretStore, VaultError};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zeroize::Zeroizing;

use crate::Connection;
use crate::encryption::{EncryptionError, IncomingVerifications, VerificationFlow};
use crate::inbox::InboxFeed;
use crate::session::{self, Session, SessionError, SignIn};
use crate::timeline::ChatTimeline;

/// How many older messages one "load older" asks for.
const PAGE: u16 = 30;
/// How often the connection and recovery state are looked at.
const TICK: Duration = Duration::from_secs(1);

/// Where the engine keeps its secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secrets {
    /// The OS keychain if it answers, else the unlock passphrase.
    System,
    /// Always the unlock passphrase, with these Argon2 settings (tests use cheap ones).
    Passphrase(KdfParams),
}

/// Calls the interface back when there is something to poll (in the desktop app, egui's repaint).
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// The running engine. Dropping it stops the sync and waits for its thread to end.
pub struct Engine {
    commands: UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Engine {
    /// Starts the engine for the data folder. It begins by finding out whether someone is signed in.
    pub fn start(data_dir: PathBuf, secrets: Secrets, wake: Wake) -> Result<Self, String> {
        let (commands, rx) = unbounded_channel();
        let (tx, events) = mpsc::channel();
        let emit = Emitter { tx, wake };
        let thread = std::thread::Builder::new()
            .name("pingcraft-matrix".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(e) => return emit.send(Event::Problem(format!("could not start: {e}"))),
                };
                runtime.block_on(Worker::new(data_dir, secrets, emit).run(rx));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { commands, events, thread: Some(thread) })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Closing the command channel ends the worker's loop, which stops the sync.
        let (closed, _) = unbounded_channel();
        self.commands = closed;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Backend for Engine {
    fn send(&mut self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn poll(&mut self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

struct Emitter {
    tx: mpsc::Sender<Event>,
    wake: Wake,
}

impl Emitter {
    fn send(&self, event: Event) {
        if self.tx.send(event).is_ok() {
            (self.wake)();
        }
    }
}

type Store = Box<dyn SecretStore + Send + Sync>;

/// Everything that exists while someone is signed in.
struct SignedIn {
    session: Session,
    connection: Connection,
    inbox: InboxFeed,
    incoming: IncomingVerifications,
    chat: Option<(ChatId, ChatTimeline)>,
    flow: Option<VerificationFlow>,
    shown_state: Option<ConnectionState>,
    shown_recovery: Option<RecoveryStatus>,
}

struct Worker {
    data_dir: PathBuf,
    secrets_choice: Secrets,
    secrets: Option<Store>,
    emit: Emitter,
    signed_in: Option<SignedIn>,
}

/// What woke the worker.
enum Woke {
    Command(Option<Command>),
    Inbox(Option<pingcraft_core::InboxChange>),
    Messages(Option<Vec<pingcraft_core::Message>>),
    Incoming(Option<VerificationFlow>),
    Verification(Option<Verification>),
    Tick,
}

/// Waits on an optional source; never finishes when there is none.
async fn maybe<T, F: Future<Output = Option<T>>>(future: Option<F>) -> Option<T> {
    match future {
        Some(f) => f.await,
        None => std::future::pending().await,
    }
}

fn profile_file(data_dir: &Path) -> PathBuf {
    data_dir.join("profile")
}

fn vault_file(data_dir: &Path) -> PathBuf {
    data_dir.join("vault")
}

impl Worker {
    fn new(data_dir: PathBuf, secrets_choice: Secrets, emit: Emitter) -> Self {
        Self { data_dir, secrets_choice, secrets: None, emit, signed_in: None }
    }

    async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        self.begin().await;
        let mut tick = tokio::time::interval(TICK);
        loop {
            let wake = {
                let s = self.signed_in.as_mut();
                let (inbox, chat, incoming, flow) = match s {
                    Some(s) => (Some(&mut s.inbox), s.chat.as_mut().map(|(_, c)| c), Some(&mut s.incoming), s.flow.as_mut()),
                    None => (None, None, None, None),
                };
                tokio::select! {
                    c = commands.recv() => Woke::Command(c),
                    Some(change) = maybe(inbox.map(|f| f.next())) => Woke::Inbox(Some(change)),
                    Some(messages) = maybe(chat.map(|c| c.next())) => Woke::Messages(Some(messages)),
                    Some(flow) = maybe(incoming.map(|i| i.next())) => Woke::Incoming(Some(flow)),
                    Some(state) = maybe(flow.map(|f| f.next())) => Woke::Verification(Some(state)),
                    _ = tick.tick() => Woke::Tick,
                }
            };
            match wake {
                Woke::Command(None) => break,
                Woke::Command(Some(command)) => self.handle(command).await,
                Woke::Inbox(Some(change)) => self.emit.send(Event::Inbox(change)),
                Woke::Messages(Some(messages)) => self.messages(messages).await,
                Woke::Incoming(Some(flow)) => self.incoming(flow),
                Woke::Verification(Some(state)) => self.emit.send(Event::Verification(Some(state))),
                Woke::Inbox(None) | Woke::Messages(None) | Woke::Incoming(None) | Woke::Verification(None) => {}
                Woke::Tick => self.tick(),
            }
        }
        if let Some(s) = self.signed_in.take() {
            s.connection.stop().await;
        }
    }

    /// Finds the secrets, then whoever was signed in.
    async fn begin(&mut self) {
        if self.secrets_choice == Secrets::System && OsKeychain.probe().is_ok() {
            self.secrets = Some(Box::new(OsKeychain));
            self.resume().await;
        } else {
            self.emit.send(Event::Locked { first: !PassphraseVault::exists(&vault_file(&self.data_dir)) });
        }
    }

    async fn unlock(&mut self, passphrase: &str) {
        if self.secrets.is_some() {
            return;
        }
        let path = vault_file(&self.data_dir);
        let params = match self.secrets_choice {
            Secrets::Passphrase(params) => params,
            Secrets::System => KdfParams::DEFAULT,
        };
        let opened = if PassphraseVault::exists(&path) {
            PassphraseVault::unlock(&path, passphrase)
        } else {
            PassphraseVault::create_with(&path, passphrase, params)
        };
        match opened {
            Ok(vault) => {
                self.secrets = Some(Box::new(vault));
                self.resume().await;
            }
            Err(VaultError::WrongPassphrase | VaultError::EmptyPassphrase) => self.emit.send(Event::WrongPassphrase),
            Err(e) => self.emit.send(Event::Problem(e.to_string())),
        }
    }

    /// Comes back to the profile signed in last time, if there is one.
    async fn resume(&mut self) {
        let Some(secrets) = self.secrets.as_deref() else { return };
        let profile = std::fs::read_to_string(profile_file(&self.data_dir)).ok().map(|p| p.trim().to_owned()).filter(|p| !p.is_empty());
        let Some(profile) = profile else { return self.emit.send(Event::SignedOut) };
        match session::restore(&self.data_dir, secrets, &profile).await {
            Ok(session) => self.connect(session, None).await,
            Err(e) => {
                self.emit.send(Event::Problem(e.to_string()));
                let _ = std::fs::remove_file(profile_file(&self.data_dir));
                self.emit.send(Event::SignedOut);
            }
        }
    }

    async fn sign_in(&mut self, server: String, username: String, password: Zeroizing<String>) {
        let Some(secrets) = self.secrets.as_deref() else { return };
        if self.signed_in.is_some() {
            return;
        }
        self.emit.send(Event::SigningIn);
        let request = SignIn { server, username, password };
        match session::sign_in(&self.data_dir, secrets, &request).await {
            Ok(session) => {
                if let Err(e) = std::fs::write(profile_file(&self.data_dir), &session.profile) {
                    self.emit.send(Event::Problem(e.to_string()));
                }
                self.connect(session, Some(&request.password)).await;
            }
            Err(e) => self.emit.send(Event::SignInFailed(match e {
                SessionError::ServerNotFound(_) => SignInProblem::ServerNotFound,
                SessionError::WrongPassword => SignInProblem::WrongPassword,
                SessionError::Keychain(_) => SignInProblem::Secrets,
                other => SignInProblem::Other(other.to_string()),
            })),
        }
    }

    /// Starts syncing for a session. After a fresh sign-in (`password` given), also sets up encryption.
    async fn connect(&mut self, session: Session, password: Option<&str>) {
        let connection = match Connection::start(session.client.clone()).await {
            Ok(c) => c,
            Err(e) => return self.emit.send(Event::Problem(e.to_string())),
        };
        let inbox = match connection.inbox().await {
            Ok(i) => i,
            Err(e) => return self.emit.send(Event::Problem(e.to_string())),
        };
        let incoming = connection.incoming_verifications();
        self.emit.send(Event::SignedIn { user: session.user_id().unwrap_or_default() });
        if password.is_some() {
            match connection.set_up_encryption(password).await {
                Ok(Some(key)) => self.emit.send(Event::NewRecoveryKey(key)),
                Ok(None) | Err(EncryptionError::BackupExists) => {}
                Err(e) => self.emit.send(Event::Problem(e.to_string())),
            }
        }
        self.signed_in = Some(SignedIn { session, connection, inbox, incoming, chat: None, flow: None, shown_state: None, shown_recovery: None });
        self.tick();
    }

    /// Tells the interface when the connection or the recovery state changed.
    fn tick(&mut self) {
        let Some(s) = self.signed_in.as_mut() else { return };
        let state = s.connection.state();
        if s.shown_state.as_ref() != Some(&state) {
            s.shown_state = Some(state.clone());
            self.emit.send(Event::Connection(state));
        }
        let recovery = s.connection.recovery_status();
        if s.shown_recovery != Some(recovery) {
            s.shown_recovery = Some(recovery);
            self.emit.send(Event::Recovery(recovery));
        }
    }

    async fn messages(&mut self, messages: Vec<pingcraft_core::Message>) {
        let Some((chat, timeline)) = self.signed_in.as_ref().and_then(|s| s.chat.as_ref()) else { return };
        self.emit.send(Event::Messages { chat: chat.clone(), messages });
        // The chat is open, so what arrives is read.
        let _ = timeline.mark_read().await;
    }

    fn incoming(&mut self, flow: VerificationFlow) {
        let Some(s) = self.signed_in.as_mut() else { return };
        // One verification at a time; another device's request waits until this one is closed.
        if s.flow.is_none() {
            s.flow = Some(flow);
        }
    }

    /// Clears what this computer keeps when the unlock passphrase is forgotten: the vault, the stores and the
    /// profile name. The server session cannot be ended without the token in the vault; it stays in the user's
    /// device list until they remove it from another device.
    fn forget_this_computer(&mut self) {
        let _ = std::fs::remove_file(vault_file(&self.data_dir));
        let _ = std::fs::remove_file(profile_file(&self.data_dir));
        let _ = std::fs::remove_dir_all(self.data_dir.join("profiles"));
        self.emit.send(Event::Locked { first: true });
    }

    async fn sign_out(&mut self) {
        if self.secrets.is_none() {
            return self.forget_this_computer();
        }
        let Some(s) = self.signed_in.take() else { return self.emit.send(Event::SignedOut) };
        s.connection.stop().await;
        drop(s.chat);
        drop(s.flow);
        drop(s.inbox);
        drop(s.incoming);
        if let Some(secrets) = self.secrets.as_deref()
            && let Err(e) = session::sign_out(&self.data_dir, secrets, s.session).await
        {
            // The server may not have heard; what this device kept is gone either way.
            self.emit.send(Event::Problem(e.to_string()));
        }
        let _ = std::fs::remove_file(profile_file(&self.data_dir));
        self.emit.send(Event::SignedOut);
    }

    async fn handle(&mut self, command: Command) {
        match command {
            Command::Unlock { passphrase } => self.unlock(&passphrase).await,
            Command::SignIn { server, username, password } => self.sign_in(server, username, password).await,
            Command::SignOut => self.sign_out().await,
            other => self.handle_signed_in(other).await,
        }
    }

    async fn handle_signed_in(&mut self, command: Command) {
        let Some(s) = self.signed_in.as_mut() else { return };
        let result: Result<(), String> = match command {
            Command::OpenChat(chat) => {
                s.chat = None;
                match s.connection.open_chat(&chat).await {
                    Ok(timeline) => {
                        let _ = timeline.mark_read().await;
                        s.chat = Some((chat, timeline));
                        Ok(())
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            Command::CloseChat => {
                s.chat = None;
                Ok(())
            }
            Command::SendText { chat, text } => match &s.chat {
                Some((open, timeline)) if *open == chat => timeline.send_text(&text).await.map_err(|e| e.to_string()),
                _ => Err("that chat is not open".into()),
            },
            Command::Retry { chat, message } => match &s.chat {
                Some((open, timeline)) if *open == chat => timeline.retry(&message).await.map_err(|e| e.to_string()),
                _ => Err("that chat is not open".into()),
            },
            Command::LoadOlder(chat) => match &s.chat {
                Some((open, timeline)) if *open == chat => match timeline.load_older(PAGE).await {
                    Ok(true) => {
                        self.emit.send(Event::ReachedStart(chat));
                        Ok(())
                    }
                    Ok(false) => Ok(()),
                    Err(e) => Err(e.to_string()),
                },
                _ => Ok(()),
            },
            Command::RecoveryKeySaved => {
                s.shown_recovery = None;
                Ok(())
            }
            Command::Recover(key) => match s.connection.recover(&key).await {
                Ok(()) => {
                    s.shown_recovery = None;
                    Ok(())
                }
                Err(EncryptionError::WrongRecoveryKey) => {
                    self.emit.send(Event::WrongRecoveryKey);
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            },
            Command::VerifyThisDevice => match s.connection.verify_this_device().await {
                Ok(flow) => {
                    s.flow = Some(flow);
                    self.emit.send(Event::Verification(Some(Verification::Waiting)));
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            },
            Command::AcceptVerification => match &s.flow {
                Some(flow) => flow.accept().await.map_err(|e| e.to_string()),
                None => Ok(()),
            },
            Command::SymbolsMatch => match s.flow.as_mut() {
                Some(flow) => flow.confirm().await.map_err(|e| e.to_string()),
                None => Ok(()),
            },
            Command::SymbolsDiffer => match s.flow.as_mut() {
                Some(flow) => flow.mismatch().await.map_err(|e| e.to_string()),
                None => Ok(()),
            },
            Command::CancelVerification => {
                if let Some(mut flow) = s.flow.take() {
                    let _ = flow.cancel().await;
                }
                self.emit.send(Event::Verification(None));
                Ok(())
            }
            // Handled before.
            Command::Unlock { .. } | Command::SignIn { .. } | Command::SignOut => Ok(()),
        };
        if let Err(problem) = result {
            self.emit.send(Event::Problem(problem));
        }
        self.tick();
    }
}

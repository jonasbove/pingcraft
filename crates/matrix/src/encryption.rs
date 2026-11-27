//! End-to-end encryption: cross-signing, the key backup with its recovery key, and verifying this device from
//! another one by comparing symbols (SAS). Mapped onto `pingcraft_core::encryption`.
//!
//! The first sign-in sets up cross-signing and a recovery key (ADR 0004). At sign-in, a recovery key is only created when
//! the account has none: if the server already holds a key backup, PingCraft asks for the existing recovery key and
//! never replaces it, since that would cut the user off from their encrypted history. Settings can make a new key in
//! place of one this device can open ([`Connection::new_recovery_key`]); resetting a lost recovery key is not offered
//! yet.

use std::pin::pin;
use std::time::Duration;

use futures_util::StreamExt;
use matrix_sdk::Client;
use matrix_sdk::encryption::CrossSigningResetAuthType;
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::encryption::verification::{CancelInfo, SasState, SasVerification, VerificationRequest, VerificationRequestState};
use matrix_sdk::event_handler::EventHandlerDropGuard;
use matrix_sdk::executor::{AbortOnDrop, spawn};
use matrix_sdk::ruma::api::client::uiaa::{AuthData, MatrixUserIdentifier, Password, UserIdentifier};
use matrix_sdk::ruma::events::key::verification::cancel::CancelCode;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::timeout::timeout;
use pingcraft_core::encryption::{CancelReason, RecoveryKey, RecoveryStatus, SasSymbol, Verification};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::connection::Connection;

/// How long to wait for the first sync to say whether the account has a recovery key.
const STATE_WAIT: Duration = Duration::from_secs(30);

/// Why setting up or using encryption failed.
#[derive(Debug)]
pub enum EncryptionError {
    /// The server wants the user's password to set up cross-signing; ask for it and call again.
    NeedsPassword,
    /// The account already has a key backup that this device cannot open: ask for the existing recovery key.
    BackupExists,
    /// That recovery key does not open the account's key backup.
    WrongRecoveryKey,
    /// The account has no cross-signing identity yet, so there is nothing to verify against.
    NotSetUp,
    /// The server did not take the account password given.
    WrongPassword,
    /// The server wants another way of proving it is the user (single sign-on), which PingCraft cannot do yet.
    NotSupported,
    /// Anything else the SDK reported, for the log. Never contains a key.
    Sdk(String),
}

impl std::fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedsPassword => f.write_str("the server asks for the password to set up encryption"),
            Self::BackupExists => f.write_str("the account already has a key backup; enter its recovery key"),
            Self::WrongRecoveryKey => f.write_str("that recovery key does not open the key backup"),
            Self::NotSetUp => f.write_str("encryption has not been set up for this account yet"),
            Self::WrongPassword => f.write_str("the server did not accept that password"),
            Self::NotSupported => f.write_str("the server wants another way of signing in for this, which PingCraft cannot do yet"),
            Self::Sdk(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for EncryptionError {}

fn sdk(e: impl std::fmt::Display) -> EncryptionError {
    EncryptionError::Sdk(e.to_string())
}

fn status(state: RecoveryState) -> RecoveryStatus {
    match state {
        RecoveryState::Unknown => RecoveryStatus::Unknown,
        RecoveryState::Disabled => RecoveryStatus::Off,
        RecoveryState::Enabled => RecoveryStatus::On,
        RecoveryState::Incomplete => RecoveryStatus::NeedsKey,
    }
}

impl Connection {
    /// Whether this device can read the account's encrypted history.
    pub fn recovery_status(&self) -> RecoveryStatus {
        status(self.client().encryption().recovery().state())
    }

    /// Waits until the sync has said whether the account has a recovery key (at most 30 seconds).
    pub async fn known_recovery_status(&self) -> RecoveryStatus {
        let recovery = self.client().encryption().recovery();
        if recovery.state() != RecoveryState::Unknown {
            return status(recovery.state());
        }
        let mut states = pin!(recovery.state_stream());
        let known = timeout(
            async {
                while let Some(state) = states.next().await {
                    if state != RecoveryState::Unknown {
                        return status(state);
                    }
                }
                RecoveryStatus::Unknown
            },
            STATE_WAIT,
        )
        .await;
        known.unwrap_or(RecoveryStatus::Unknown)
    }

    /// Whether another of the user's devices (or the recovery key) has vouched for this one.
    pub async fn is_this_device_verified(&self) -> bool {
        matches!(self.client().encryption().get_own_device().await, Ok(Some(device)) if device.is_cross_signed_by_owner())
    }

    /// Sets up encryption after the first sign-in: cross-signing, then a key backup and a new recovery key, which
    /// the user must save. Returns `None` when there was nothing to set up. Never replaces an existing backup:
    /// that is [`EncryptionError::BackupExists`]. `password` is the one the user just signed in with, for servers
    /// that ask for it again.
    pub async fn set_up_encryption(&self, password: Option<&str>) -> Result<Option<RecoveryKey>, EncryptionError> {
        let client = self.client();
        let encryption = client.encryption();
        encryption.wait_for_e2ee_initialization_tasks().await;
        bootstrap_cross_signing(client, password).await?;

        match self.known_recovery_status().await {
            RecoveryStatus::On => Ok(None),
            RecoveryStatus::NeedsKey => Err(EncryptionError::BackupExists),
            RecoveryStatus::Unknown => Err(EncryptionError::Sdk("the server did not say whether a recovery key exists".into())),
            RecoveryStatus::Off => {
                if encryption.backups().fetch_exists_on_server().await.map_err(sdk)? {
                    return Err(EncryptionError::BackupExists);
                }
                let key = encryption.recovery().enable().wait_for_backups_to_upload().await.map_err(sdk)?;
                Ok(Some(RecoveryKey::new(key)))
            }
        }
    }

    /// Makes a new recovery key, for Settings: in place of the account's when this device can open the key backup
    /// (the backup stays, and the old key stops working), or the first one when the account has none. A first key
    /// sets up cross-signing first when the account has none (`password` is the account password, for servers that
    /// ask for it): a key holds the cross-signing secrets, and without them this device could not use the key it
    /// just made. Refused when the account has a backup this device cannot open ([`EncryptionError::BackupExists`]:
    /// [`Connection::reset_recovery`] is for that), and while the state is not known yet. Succeeds only once this
    /// device can read the backup with the new key ([`RecoveryStatus::On`]).
    pub async fn new_recovery_key(&self, password: Option<&str>) -> Result<RecoveryKey, EncryptionError> {
        let encryption = self.client().encryption();
        let key = match self.recovery_status() {
            RecoveryStatus::On => encryption.recovery().reset_key().await.map_err(sdk)?,
            RecoveryStatus::Off => {
                bootstrap_cross_signing(self.client(), password).await.map_err(|e| wrong_password(e, password))?;
                if encryption.backups().fetch_exists_on_server().await.map_err(sdk)? {
                    return Err(EncryptionError::BackupExists);
                }
                encryption.recovery().enable().wait_for_backups_to_upload().await.map_err(sdk)?
            }
            RecoveryStatus::NeedsKey => return Err(EncryptionError::BackupExists),
            RecoveryStatus::Unknown => return Err(EncryptionError::Sdk("the server has not said yet whether a recovery key exists".into())),
        };
        key_made(self.recovery_status()).map(|()| RecoveryKey::new(key))
    }

    /// Starts over on a device that cannot open the key backup (the recovery key is lost): deletes the backup and
    /// the secret storage, makes a new cross-signing identity (the server asks for the account `password`), and
    /// makes a new recovery key, the matrix-rust-sdk way (`Recovery::reset_identity`, then `enable`). Messages
    /// encrypted before can no longer be read on devices that do not already have their keys, and the user's other
    /// devices need verifying again.
    pub async fn reset_recovery(&self, password: &str) -> Result<RecoveryKey, EncryptionError> {
        let client = self.client();
        let recovery = client.encryption().recovery();
        if let Some(handle) = recovery.reset_identity().await.map_err(sdk)? {
            match handle.auth_type() {
                CrossSigningResetAuthType::Uiaa(uiaa) => {
                    let user = client.user_id().ok_or(EncryptionError::NotSetUp)?;
                    let mut auth = Password::new(UserIdentifier::Matrix(MatrixUserIdentifier::new(user.to_string())), password.to_owned());
                    auth.session = uiaa.session.clone();
                    handle.reset(Some(AuthData::Password(auth))).await.map_err(|e| match &e {
                        matrix_sdk::encryption::recovery::RecoveryError::Sdk(inner) if inner.as_uiaa_response().is_some() => {
                            EncryptionError::WrongPassword
                        }
                        _ => sdk(e),
                    })?;
                }
                CrossSigningResetAuthType::OAuth(_) => {
                    handle.cancel().await;
                    return Err(EncryptionError::NotSupported);
                }
            }
        }
        let key = recovery.enable().wait_for_backups_to_upload().await.map_err(sdk)?;
        key_made(self.recovery_status()).map(|()| RecoveryKey::new(key))
    }

    /// Gives this device the account's secrets from the key backup, with the recovery key the user typed.
    pub async fn recover(&self, key: &RecoveryKey) -> Result<(), EncryptionError> {
        self.client().encryption().recovery().recover(key.expose().trim()).await.map_err(|e| match e {
            matrix_sdk::encryption::recovery::RecoveryError::SecretStorage(_) => EncryptionError::WrongRecoveryKey,
            other => sdk(other),
        })
    }

    /// Asks the user's other devices to verify this one. The flow starts in [`Verification::Waiting`].
    ///
    /// A device that has not yet seen this one's keys (in the first seconds after signing in) ignores the request,
    /// and the flow stays waiting; the interface should let the user cancel and ask again.
    pub async fn verify_this_device(&self) -> Result<VerificationFlow, EncryptionError> {
        let client = self.client();
        let own = client.user_id().ok_or(EncryptionError::NotSetUp)?.to_owned();
        let encryption = client.encryption();
        // A new device may not have fetched the account's identity yet: ask the server when it is not known.
        let identity = match encryption.get_user_identity(&own).await.map_err(sdk)? {
            Some(identity) => identity,
            None => encryption.request_user_identity(&own).await.map_err(sdk)?.ok_or(EncryptionError::NotSetUp)?,
        };
        let request = identity.request_verification().await.map_err(sdk)?;
        Ok(VerificationFlow::new(request))
    }

    /// Requests from the user's other devices to verify one another, as they arrive.
    pub fn incoming_verifications(&self) -> IncomingVerifications {
        let (tx, rx) = unbounded_channel();
        let handle = self.client().add_event_handler(move |event: ToDeviceKeyVerificationRequestEvent, client: Client| {
            let tx = tx.clone();
            async move {
                // Only the user's own devices: verifying other people comes later.
                if client.user_id() != Some(event.sender.as_ref()) {
                    return;
                }
                if let Some(request) = client.encryption().get_verification_request(&event.sender, &event.content.transaction_id).await {
                    let _ = tx.send(request);
                }
            }
        });
        IncomingVerifications { requests: rx, _guard: self.client().event_handler_drop_guard(handle) }
    }
}

/// A key just made is only a success once this device can read the backup with it: otherwise the user would save a
/// key and still be asked for one.
fn key_made(after: RecoveryStatus) -> Result<(), EncryptionError> {
    match after {
        RecoveryStatus::On => Ok(()),
        other => Err(EncryptionError::Sdk(format!("the new recovery key does not hold everything this device needs ({other:?})"))),
    }
}

/// The server asking again for a password that was given means it did not take it.
fn wrong_password(e: EncryptionError, password: Option<&str>) -> EncryptionError {
    match (e, password) {
        (EncryptionError::NeedsPassword, Some(_)) => EncryptionError::WrongPassword,
        (e, _) => e,
    }
}

async fn bootstrap_cross_signing(client: &Client, password: Option<&str>) -> Result<(), EncryptionError> {
    let encryption = client.encryption();
    let Err(error) = encryption.bootstrap_cross_signing_if_needed(None).await else { return Ok(()) };
    let Some(uiaa) = error.as_uiaa_response() else { return Err(sdk(error)) };
    let password = password.ok_or(EncryptionError::NeedsPassword)?;
    let user = client.user_id().ok_or(EncryptionError::NotSetUp)?;
    let mut auth = Password::new(UserIdentifier::Matrix(MatrixUserIdentifier::new(user.to_string())), password.to_owned());
    auth.session = uiaa.session.clone();
    encryption
        .bootstrap_cross_signing(Some(AuthData::Password(auth)))
        .await
        .map_err(|e| if e.as_uiaa_response().is_some() { EncryptionError::NeedsPassword } else { sdk(e) })
}

/// Verification requests from the user's other devices. Dropping it stops listening.
pub struct IncomingVerifications {
    requests: UnboundedReceiver<VerificationRequest>,
    _guard: EventHandlerDropGuard,
}

impl IncomingVerifications {
    /// The next request, if one arrived. Never blocks.
    pub fn try_next(&mut self) -> Option<VerificationFlow> {
        self.requests.try_recv().ok().map(VerificationFlow::new)
    }

    /// Waits for the next request.
    pub async fn next(&mut self) -> Option<VerificationFlow> {
        self.requests.recv().await.map(VerificationFlow::new)
    }
}

/// One verification between this device and another of the user's. Poll [`VerificationFlow::try_next`] for its
/// state; answer with the methods below.
pub struct VerificationFlow {
    request: VerificationRequest,
    /// The comparison, once the request has turned into one; the task that follows it sends it here.
    sas: Option<SasVerification>,
    sas_rx: UnboundedReceiver<SasVerification>,
    states: UnboundedReceiver<Verification>,
    _task: AbortOnDrop<()>,
}

impl VerificationFlow {
    pub(crate) fn new(request: VerificationRequest) -> Self {
        let (sas_tx, sas_rx) = unbounded_channel();
        let (tx, states) = unbounded_channel();
        let task = spawn(follow(request.clone(), sas_tx, tx));
        Self { request, sas: None, sas_rx, states, _task: AbortOnDrop::new(task) }
    }

    /// The newest state, if it changed. Never blocks.
    pub fn try_next(&mut self) -> Option<Verification> {
        let mut newest = None;
        while let Ok(state) = self.states.try_recv() {
            newest = Some(state);
        }
        newest
    }

    /// Waits for the next state. `None` once the flow has ended and every state was read.
    pub async fn next(&mut self) -> Option<Verification> {
        self.states.recv().await
    }

    fn current_sas(&mut self) -> Option<SasVerification> {
        while let Ok(sas) = self.sas_rx.try_recv() {
            self.sas = Some(sas);
        }
        self.sas.clone()
    }

    /// Accepts a request from another device.
    pub async fn accept(&self) -> Result<(), EncryptionError> {
        self.request.accept().await.map_err(sdk)
    }

    /// The user says the symbols match.
    pub async fn confirm(&mut self) -> Result<(), EncryptionError> {
        self.current_sas().ok_or(EncryptionError::NotSetUp)?.confirm().await.map_err(sdk)
    }

    /// The user says the symbols do not match. Nothing is trusted, and the other device is told.
    pub async fn mismatch(&mut self) -> Result<(), EncryptionError> {
        self.current_sas().ok_or(EncryptionError::NotSetUp)?.mismatch().await.map_err(sdk)
    }

    /// Stops the verification (or declines a request).
    pub async fn cancel(&mut self) -> Result<(), EncryptionError> {
        match self.current_sas() {
            Some(sas) => sas.cancel().await.map_err(sdk),
            None => self.request.cancel().await.map_err(sdk),
        }
    }
}

fn reason(info: &CancelInfo) -> CancelReason {
    match info.cancel_code() {
        CancelCode::MismatchedSas | CancelCode::KeyMismatch | CancelCode::MismatchedCommitment => CancelReason::Mismatch,
        CancelCode::User | CancelCode::Accepted => CancelReason::Cancelled,
        CancelCode::Timeout => CancelReason::TimedOut,
        _ => CancelReason::Other(info.reason().to_owned()),
    }
}

/// Follows a request, and the SAS verification it turns into, sending each state to the flow.
async fn follow(request: VerificationRequest, slot: UnboundedSender<SasVerification>, tx: UnboundedSender<Verification>) {
    let mut changes = pin!(request.changes());
    let mut state = request.state();
    loop {
        let sas = match state {
            VerificationRequestState::Created { .. } => {
                let _ = tx.send(Verification::Waiting);
                None
            }
            VerificationRequestState::Requested { other_device_data, .. } => {
                let device = other_device_data.display_name().map(str::to_owned).unwrap_or_else(|| other_device_data.device_id().to_string());
                let _ = tx.send(if request.we_started() { Verification::Waiting } else { Verification::Incoming { device } });
                None
            }
            VerificationRequestState::Ready { .. } => {
                let _ = tx.send(Verification::Waiting);
                // The device that asked starts the comparison once the other has accepted.
                if request.we_started() {
                    match request.start_sas().await {
                        Ok(sas) => sas,
                        Err(e) => {
                            let _ = tx.send(Verification::Cancelled(CancelReason::Other(e.to_string())));
                            return;
                        }
                    }
                } else {
                    None
                }
            }
            VerificationRequestState::Transitioned { verification, .. } => verification.sas(),
            VerificationRequestState::Done => {
                let _ = tx.send(Verification::Done);
                return;
            }
            VerificationRequestState::Cancelled(info) => {
                let _ = tx.send(Verification::Cancelled(reason(&info)));
                return;
            }
        };
        if let Some(sas) = sas {
            follow_sas(sas, &slot, &tx).await;
            return;
        }
        match changes.next().await {
            Some(next) => state = next,
            None => return,
        }
    }
}

async fn follow_sas(sas: SasVerification, slot: &UnboundedSender<SasVerification>, tx: &UnboundedSender<Verification>) {
    let _ = slot.send(sas.clone());
    if !sas.we_started()
        && let Err(e) = sas.accept().await
    {
        let _ = tx.send(Verification::Cancelled(CancelReason::Other(e.to_string())));
        return;
    }
    let mut changes = pin!(sas.changes());
    let mut state = sas.state();
    loop {
        let shown = match state {
            SasState::Created { .. } | SasState::Started { .. } | SasState::Accepted { .. } => Verification::Waiting,
            SasState::KeysExchanged { emojis: Some(emojis), .. } => Verification::Compare(
                emojis
                    .indices
                    .iter()
                    .zip(emojis.emojis.iter())
                    .map(|(index, e)| SasSymbol { index: *index, symbol: e.symbol.to_owned(), name: e.description.to_owned() })
                    .collect(),
            ),
            SasState::KeysExchanged { emojis: None, .. } => {
                // Every Matrix client that verifies by emoji supports it; numbers only are not offered.
                let _ = sas.cancel().await;
                Verification::Cancelled(CancelReason::Other("the other device cannot compare symbols".into()))
            }
            SasState::Confirmed => Verification::Confirmed,
            SasState::Done { .. } => Verification::Done,
            SasState::Cancelled(info) => Verification::Cancelled(reason(&info)),
        };
        let finished = matches!(shown, Verification::Done | Verification::Cancelled(_));
        let _ = tx.send(shown);
        if finished {
            return;
        }
        match changes.next().await {
            Some(next) => state = next,
            None => return,
        }
    }
}

#[cfg(test)]
mod key_tests {
    use pingcraft_core::encryption::RecoveryStatus;

    use super::{EncryptionError, key_made, wrong_password};

    /// A new key is reported only once this device can use it: a key left "incomplete" (the cross-signing secrets
    /// missing from it, as a first key made without cross-signing was) is an error, not a key to save.
    #[test]
    fn a_new_key_counts_only_when_this_device_can_use_it() {
        assert!(key_made(RecoveryStatus::On).is_ok());
        for other in [RecoveryStatus::NeedsKey, RecoveryStatus::Off, RecoveryStatus::Unknown] {
            assert!(matches!(key_made(other), Err(EncryptionError::Sdk(_))), "{other:?}");
        }
    }

    #[test]
    fn asked_again_for_a_given_password_means_it_was_wrong() {
        assert!(matches!(wrong_password(EncryptionError::NeedsPassword, Some("pw")), EncryptionError::WrongPassword));
        assert!(matches!(wrong_password(EncryptionError::NeedsPassword, None), EncryptionError::NeedsPassword));
    }
}

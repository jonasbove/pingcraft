//! The user's devices (Matrix sessions), for Settings: the list with each one's name, when it was last seen and
//! whether it is verified; renaming this one; verifying another by comparing symbols; and signing another out, which
//! the server allows only after the user proves it is them again (user-interactive authentication, with the account
//! password). The last-seen IP address is never read out of the response into PingCraft.

use matrix_sdk::ruma::OwnedDeviceId;
use matrix_sdk::ruma::api::client::uiaa::{AuthData, AuthType, MatrixUserIdentifier, Password, UserIdentifier};
use pingcraft_core::Timestamp;
use pingcraft_core::devices::{Device, DeviceId, SignOutProblem, clean_name, sorted};

use crate::connection::Connection;
use crate::encryption::{EncryptionError, VerificationFlow};

/// Why the device list could not be read or a device changed, for the problem bar.
fn problem(what: &str, e: impl std::fmt::Display) -> String {
    format!("{what}: {e}")
}

impl Connection {
    /// The devices the user is signed in on, this one first ([`sorted`]).
    pub async fn devices(&self) -> Result<Vec<Device>, String> {
        let client = self.client();
        let listed = client.devices().await.map_err(|e| problem("the device list could not be read", e))?;
        let own = client.user_id().map(ToOwned::to_owned);
        let this = client.device_id().map(ToOwned::to_owned);
        let mut devices = Vec::with_capacity(listed.devices.len());
        for d in listed.devices {
            let verified = match &own {
                Some(user) => {
                    matches!(client.encryption().get_device(user, &d.device_id).await, Ok(Some(device)) if device.is_cross_signed_by_owner())
                }
                None => false,
            };
            devices.push(Device {
                this: this.as_deref() == Some(d.device_id.as_ref()),
                id: DeviceId(d.device_id.to_string()),
                name: d.display_name.unwrap_or_default(),
                last_seen: d.last_seen_ts.map(|ts| Timestamp(u64::from(ts.0))),
                verified,
            });
        }
        Ok(sorted(devices))
    }

    /// Gives this device a new name, which the user's other devices see. A name that is empty once trimmed is refused.
    pub async fn rename_this_device(&self, typed: &str) -> Result<(), String> {
        let name = clean_name(typed).ok_or_else(|| "a device needs a name".to_owned())?;
        let client = self.client();
        let id = client.device_id().ok_or_else(|| "this device has no ID yet".to_owned())?;
        client.rename_device(id, &name).await.map(|_| ()).map_err(|e| problem("the device could not be renamed", e))
    }

    /// Asks another of the user's devices to verify with this one. The flow starts in `Verification::Waiting`.
    pub async fn verify_device(&self, device: &DeviceId) -> Result<VerificationFlow, EncryptionError> {
        let client = self.client();
        let user = client.user_id().ok_or(EncryptionError::NotSetUp)?.to_owned();
        let id: OwnedDeviceId = device.0.as_str().into();
        let found = client.encryption().get_device(&user, &id).await.map_err(|e| EncryptionError::Sdk(e.to_string()))?;
        let found = found.ok_or_else(|| EncryptionError::Sdk("that device's keys are not known yet".into()))?;
        let request = found.request_verification().await.map_err(|e| EncryptionError::Sdk(e.to_string()))?;
        Ok(VerificationFlow::new(request))
    }

    /// Signs another of the user's devices out, proving it is the user with the account password. This device is
    /// refused here: signing it out is the engine's sign-out, which also clears what this computer keeps.
    pub async fn sign_out_device(&self, device: &DeviceId, password: &str) -> Result<(), SignOutProblem> {
        let client = self.client();
        if client.device_id().is_some_and(|this| this.as_str() == device.0) {
            return Err(SignOutProblem::Other("this device signs out with Sign out".into()));
        }
        let user = client.user_id().ok_or_else(|| SignOutProblem::Other("not signed in".into()))?.to_owned();
        let ids = [OwnedDeviceId::from(device.0.as_str())];
        // The first request has no proof; the server answers with the ways it accepts.
        let error = match client.delete_devices(&ids, None).await {
            Ok(_) => return Ok(()),
            Err(error) => error,
        };
        let Some(uiaa) = error.as_uiaa_response() else { return Err(SignOutProblem::Other(error.to_string())) };
        let by_password = uiaa.flows.iter().any(|flow| flow.stages.first() == Some(&AuthType::Password));
        if !by_password {
            return Err(SignOutProblem::NotSupported);
        }
        let mut auth = Password::new(UserIdentifier::Matrix(MatrixUserIdentifier::new(user.to_string())), password.to_owned());
        auth.session = uiaa.session.clone();
        match client.delete_devices(&ids, Some(AuthData::Password(auth))).await {
            Ok(_) => Ok(()),
            // Asked again: the password was not taken.
            Err(e) if e.as_uiaa_response().is_some() => Err(SignOutProblem::WrongPassword),
            Err(e) => Err(SignOutProblem::Other(e.to_string())),
        }
    }
}

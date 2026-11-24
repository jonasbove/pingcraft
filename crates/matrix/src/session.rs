//! Signing in, and coming back signed in (desktop).
//!
//! Each sign-in is a *profile*: a random ID that names its store folder and its two keychain entries, the store
//! key and the Matrix session (which holds the access token). The folder name says nothing about the account. The
//! stores are encrypted with the store key (ADR 0004); the session and key live only in the keychain, or, where
//! there is none, in a `PassphraseVault` the user unlocks with a passphrase. Nothing here logs a password, a token
//! or a key.

use std::path::{Path, PathBuf};

use matrix_sdk::Client;
use matrix_sdk::SqliteStoreConfig;
use matrix_sdk::authentication::matrix::MatrixSession;
use pingcraft_keychain::{KeychainError, SecretStore, new_profile_id, store_key};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// The name PingCraft gives its device on the server, so the user can tell it apart in their device list.
pub const DEVICE_NAME: &str = "PingCraft";

/// What the user typed on the sign-in screen.
pub struct SignIn {
    /// A server name (`example.org`) or the homeserver's address (`https://matrix.example.org`).
    pub server: String,
    /// A user name (`ana`) or a full Matrix ID (`@ana:example.org`).
    pub username: String,
    pub password: Zeroizing<String>,
}

/// Why signing in or coming back failed, in terms the interface can turn into plain words.
#[derive(Debug)]
pub enum SessionError {
    /// The server could not be found or is not a Matrix server.
    ServerNotFound(String),
    /// The server refused the user name and password.
    WrongPassword,
    /// The keychain could not keep or give back the secrets; the app should offer the unlock passphrase.
    Keychain(KeychainError),
    /// The local store could not be opened or written.
    Store(String),
    /// Anything else the server or the network said, for the log.
    Other(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerNotFound(why) => write!(f, "no Matrix server found there: {why}"),
            Self::WrongPassword => f.write_str("the server did not accept that user name and password"),
            Self::Keychain(e) => write!(f, "{e}"),
            Self::Store(why) => write!(f, "the local store could not be opened: {why}"),
            Self::Other(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<KeychainError> for SessionError {
    fn from(e: KeychainError) -> Self {
        Self::Keychain(e)
    }
}

/// What is saved in the keychain for a profile: the session (with its access token) and where the server is.
#[derive(Serialize, Deserialize)]
struct SavedSession {
    homeserver: String,
    session: MatrixSession,
}

fn session_name(profile: &str) -> String {
    format!("session:{profile}")
}

/// A signed-in client and the profile it belongs to.
pub struct Session {
    pub profile: String,
    pub client: Client,
}

impl Session {
    pub fn user_id(&self) -> Option<String> {
        self.client.user_id().map(ToString::to_string)
    }
}

fn store_dir(data_dir: &Path, profile: &str) -> PathBuf {
    data_dir.join("profiles").join(profile)
}

async fn client_for(data_dir: &Path, secrets: &dyn SecretStore, profile: &str, server: &str) -> Result<Client, SessionError> {
    let key = store_key(secrets, profile)?;
    let dir = store_dir(data_dir, profile);
    std::fs::create_dir_all(&dir).map_err(|e| SessionError::Store(e.to_string()))?;
    let config = SqliteStoreConfig::new(&dir).key(Some(key.bytes()));
    Client::builder().server_name_or_homeserver_url(server).sqlite_store_with_config_and_cache_path(config, None::<&Path>).build().await.map_err(
        |e| match e {
            matrix_sdk::ClientBuildError::AutoDiscovery(_) | matrix_sdk::ClientBuildError::Url(_) => SessionError::ServerNotFound(e.to_string()),
            other => SessionError::Store(other.to_string()),
        },
    )
}

/// Signs in with a user name and password, as a new profile. On success the session is in the keychain, so
/// [`restore`] can come back to it after a restart.
pub async fn sign_in(data_dir: &Path, secrets: &dyn SecretStore, request: &SignIn) -> Result<Session, SessionError> {
    let profile = new_profile_id()?;
    let client = match client_for(data_dir, secrets, &profile, request.server.trim()).await {
        Ok(client) => client,
        Err(e) => {
            forget_profile(data_dir, secrets, &profile);
            return Err(e);
        }
    };
    let login = client.matrix_auth().login_username(request.username.trim(), &request.password).initial_device_display_name(DEVICE_NAME).await;
    if let Err(e) = login {
        forget_profile(data_dir, secrets, &profile);
        return Err(match e.as_client_api_error().map(|c| c.status_code.as_u16()) {
            Some(403) => SessionError::WrongPassword,
            _ => SessionError::Other(e.to_string()),
        });
    }
    let Some(session) = client.matrix_auth().session() else {
        forget_profile(data_dir, secrets, &profile);
        return Err(SessionError::Other("signed in, but the server returned no session".into()));
    };
    let saved = SavedSession { homeserver: client.homeserver().to_string(), session };
    let json = Zeroizing::new(serde_json::to_vec(&saved).map_err(|e| SessionError::Other(e.to_string()))?);
    if let Err(e) = secrets.set(&session_name(&profile), &json) {
        forget_profile(data_dir, secrets, &profile);
        return Err(e.into());
    }
    Ok(Session { profile, client })
}

/// Comes back to a profile signed in earlier, from its keychain entries and its store.
pub async fn restore(data_dir: &Path, secrets: &dyn SecretStore, profile: &str) -> Result<Session, SessionError> {
    let json = secrets.get(&session_name(profile))?.ok_or(SessionError::Keychain(KeychainError::Corrupt("no saved session")))?;
    let saved: SavedSession = serde_json::from_slice(&json).map_err(|_| SessionError::Keychain(KeychainError::Corrupt("saved session")))?;
    let client = client_for(data_dir, secrets, profile, &saved.homeserver).await?;
    client.restore_session(saved.session).await.map_err(|e| SessionError::Other(e.to_string()))?;
    Ok(Session { profile: profile.to_owned(), client })
}

/// Signs out: ends the session on the server, then removes the profile's keychain entries and its store.
pub async fn sign_out(data_dir: &Path, secrets: &dyn SecretStore, session: Session) -> Result<(), SessionError> {
    let ended = session.client.logout().await;
    forget_profile(data_dir, secrets, &session.profile);
    ended.map_err(|e| SessionError::Other(e.to_string()))
}

/// Removes everything a profile left behind. Best effort: a half-made profile is cleaned up on the way out.
fn forget_profile(data_dir: &Path, secrets: &dyn SecretStore, profile: &str) {
    let _ = secrets.delete(&session_name(profile));
    let _ = secrets.delete(&pingcraft_keychain::store_key_name(profile));
    let _ = std::fs::remove_dir_all(store_dir(data_dir, profile));
}

#[cfg(test)]
mod tests {
    //! Against a real homeserver, when one is given (the `deploy/` stack works):
    //! `PINGCRAFT_TEST_HOMESERVER=http://127.0.0.1:8008 PINGCRAFT_TEST_USER=… PINGCRAFT_TEST_PASSWORD=…`.
    //! Without them, the tests that need a server are skipped and say so.
    use super::{SessionError, SignIn, restore, sign_in, sign_out};
    use pingcraft_keychain::{KdfParams, MemoryStore, PassphraseVault, SecretStore, store_key_name};
    use zeroize::Zeroizing;

    fn server() -> Option<(String, String, String)> {
        Some((
            std::env::var("PINGCRAFT_TEST_HOMESERVER").ok()?,
            std::env::var("PINGCRAFT_TEST_USER").ok()?,
            std::env::var("PINGCRAFT_TEST_PASSWORD").ok()?,
        ))
    }

    #[tokio::test]
    async fn sign_in_restore_and_sign_out() {
        let Some((hs, user, password)) = server() else {
            eprintln!("skipped: PINGCRAFT_TEST_HOMESERVER, _USER and _PASSWORD are not set");
            return;
        };
        let data = tempfile::tempdir().unwrap();
        let secrets = MemoryStore::default();
        let request = SignIn { server: hs, username: user, password: Zeroizing::new(password) };
        let session = sign_in(data.path(), &secrets, &request).await.unwrap();
        let profile = session.profile.clone();
        let user_id = session.user_id().unwrap();
        drop(session);

        let back = restore(data.path(), &secrets, &profile).await.unwrap();
        assert_eq!(back.user_id().unwrap(), user_id);

        sign_out(data.path(), &secrets, back).await.unwrap();
        assert!(secrets.get(&store_key_name(&profile)).unwrap().is_none(), "the store key is gone");
        assert!(!data.path().join("profiles").join(&profile).exists(), "the store is gone");
    }

    #[tokio::test]
    async fn without_a_keychain_the_unlock_passphrase_keeps_the_session() {
        let Some((hs, user, password)) = server() else {
            eprintln!("skipped: PINGCRAFT_TEST_HOMESERVER, _USER and _PASSWORD are not set");
            return;
        };
        let data = tempfile::tempdir().unwrap();
        let vault_path = data.path().join("vault");
        let cheap = KdfParams { memory_kib: 8 * 1024, iterations: 1, lanes: 1 };
        let vault = PassphraseVault::create_with(&vault_path, "an unlock passphrase", cheap).unwrap();
        let request = SignIn { server: hs, username: user, password: Zeroizing::new(password.clone()) };
        let session = sign_in(data.path(), &vault, &request).await.unwrap();
        let profile = session.profile.clone();
        drop(session);
        drop(vault);

        // Nothing secret is readable in the vault file.
        let file = std::fs::read(&vault_path).unwrap();
        assert!(!file.windows(profile.len()).any(|w| w == profile.as_bytes()), "not even the profile's name");

        assert!(PassphraseVault::unlock(&vault_path, "not it").is_err());
        let vault = PassphraseVault::unlock(&vault_path, "an unlock passphrase").unwrap();
        let back = restore(data.path(), &vault, &profile).await.unwrap();
        sign_out(data.path(), &vault, back).await.unwrap();
        assert!(vault.get(&store_key_name(&profile)).unwrap().is_none());
    }

    #[tokio::test]
    async fn a_wrong_password_leaves_nothing_behind() {
        let Some((hs, user, _)) = server() else {
            eprintln!("skipped: PINGCRAFT_TEST_HOMESERVER, _USER and _PASSWORD are not set");
            return;
        };
        let data = tempfile::tempdir().unwrap();
        let secrets = MemoryStore::default();
        let request = SignIn { server: hs, username: user, password: Zeroizing::new("not the password".into()) };
        assert!(matches!(sign_in(data.path(), &secrets, &request).await, Err(SessionError::WrongPassword)));
        let left = std::fs::read_dir(data.path().join("profiles")).map(|d| d.count()).unwrap_or(0);
        assert_eq!(left, 0, "a failed sign-in leaves no store behind");
    }

    #[tokio::test]
    async fn an_unknown_server_is_reported_as_such() {
        let data = tempfile::tempdir().unwrap();
        let secrets = MemoryStore::default();
        let request = SignIn { server: "http://127.0.0.1:9".into(), username: "ana".into(), password: Zeroizing::new("x".into()) };
        let result = sign_in(data.path(), &secrets, &request).await;
        assert!(result.is_err(), "nothing listens on port 9");
        let left = std::fs::read_dir(data.path().join("profiles")).map(|d| d.count()).unwrap_or(0);
        assert_eq!(left, 0);
    }
}

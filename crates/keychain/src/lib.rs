//! The OS keychain, for the secrets PingCraft keeps on a desktop: the key that encrypts its local stores and the
//! Matrix session (ADR 0004).
//!
//! Secrets are never written to a plain file. When there is no working keychain, [`store_key`] fails with
//! [`KeychainError::Unavailable`], and the app asks for an unlock passphrase instead. Nothing here logs or prints
//! a secret: [`StoreKey`]'s `Debug` is redacted and its bytes are wiped when it is dropped.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use std::collections::HashMap;
use std::sync::Mutex;

use zeroize::Zeroizing;

/// The keychain "service" every PingCraft secret is filed under.
pub const SERVICE: &str = "PingCraft";

/// The length of a store key in bytes (a raw key for the SDK's store cipher, no passphrase derivation).
pub const STORE_KEY_LEN: usize = 32;

#[derive(Debug)]
pub enum KeychainError {
    /// No keychain to use: no Secret Service running, access refused, or an unsupported platform. The message is
    /// the platform's, for the log; it never contains a secret.
    Unavailable(String),
    /// A secret is there but is not what PingCraft wrote (e.g. the wrong length).
    Corrupt(&'static str),
    /// The operating system could not supply random bytes.
    Random(String),
}

impl std::fmt::Display for KeychainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "the system keychain is not available: {why}"),
            Self::Corrupt(what) => write!(f, "a PingCraft secret in the keychain is damaged: {what}"),
            Self::Random(why) => write!(f, "no random numbers from the system: {why}"),
        }
    }
}

impl std::error::Error for KeychainError {}

/// Somewhere to keep secrets by name. The OS keychain on a desktop; memory in tests.
pub trait SecretStore {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, KeychainError>;
    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeychainError>;
    /// Removes a secret; removing one that is not there is not an error.
    fn delete(&self, name: &str) -> Result<(), KeychainError>;
}

/// The key that encrypts one account's local stores. Its bytes are wiped when it is dropped.
pub struct StoreKey(Zeroizing<[u8; STORE_KEY_LEN]>);

impl StoreKey {
    pub fn bytes(&self) -> &[u8; STORE_KEY_LEN] {
        &self.0
    }
}

impl std::fmt::Debug for StoreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoreKey(<redacted>)")
    }
}

/// The keychain name of the store key for one profile (one sign-in, with its own store folder).
pub fn store_key_name(profile: &str) -> String {
    format!("store-key:{profile}")
}

/// Returns the store key for `profile`, creating and saving a new random one the first time.
pub fn store_key(store: &dyn SecretStore, profile: &str) -> Result<StoreKey, KeychainError> {
    let name = store_key_name(profile);
    if let Some(found) = store.get(&name)? {
        let bytes: [u8; STORE_KEY_LEN] = found.as_slice().try_into().map_err(|_| KeychainError::Corrupt("store key has the wrong length"))?;
        return Ok(StoreKey(Zeroizing::new(bytes)));
    }
    let mut bytes = Zeroizing::new([0u8; STORE_KEY_LEN]);
    getrandom::fill(bytes.as_mut()).map_err(|e| KeychainError::Random(e.to_string()))?;
    store.set(&name, bytes.as_ref())?;
    Ok(StoreKey(bytes))
}

/// A new random profile name: 32 hex digits. It names a store folder and its keychain entries, and says nothing
/// about the account.
pub fn new_profile_id() -> Result<String, KeychainError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| KeychainError::Random(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Secret name -> secret, wiped when removed or dropped.
type Secrets = HashMap<String, Zeroizing<Vec<u8>>>;

/// Secrets in memory, for tests and for a session that must not touch the keychain.
#[derive(Default)]
pub struct MemoryStore {
    secrets: Mutex<Secrets>,
}

impl MemoryStore {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Secrets>, KeychainError> {
        self.secrets.lock().map_err(|_| KeychainError::Unavailable("memory store poisoned".into()))
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, KeychainError> {
        Ok(self.lock()?.get(name).cloned())
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeychainError> {
        self.lock()?.insert(name.to_owned(), Zeroizing::new(secret.to_vec()));
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), KeychainError> {
        self.lock()?.remove(name);
        Ok(())
    }
}

/// The operating system's keychain: Keychain on macOS, Credential Manager on Windows, the Secret Service on Linux
/// and the BSDs (`keyring`'s default stores).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct OsKeychain;

#[cfg(not(target_arch = "wasm32"))]
impl OsKeychain {
    fn entry(name: &str) -> Result<keyring::Entry, KeychainError> {
        keyring::Entry::new(SERVICE, name).map_err(|e| KeychainError::Unavailable(e.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SecretStore for OsKeychain {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, KeychainError> {
        match Self::entry(name)?.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeychainError::Unavailable(e.to_string())),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeychainError> {
        Self::entry(name)?.set_secret(secret).map_err(|e| KeychainError::Unavailable(e.to_string()))
    }

    fn delete(&self, name: &str) -> Result<(), KeychainError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeychainError::Unavailable(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{KeychainError, MemoryStore, STORE_KEY_LEN, SecretStore, store_key, store_key_name};

    #[test]
    fn the_store_key_is_created_once_and_then_reused() {
        let store = MemoryStore::default();
        let first = store_key(&store, "@ana:example.org").unwrap();
        let again = store_key(&store, "@ana:example.org").unwrap();
        assert_eq!(first.bytes(), again.bytes());
        assert_ne!(first.bytes(), &[0u8; STORE_KEY_LEN], "the key must be random, not zeroes");
        let other = store_key(&store, "@lars:example.org").unwrap();
        assert_ne!(first.bytes(), other.bytes(), "each account gets its own key");
    }

    #[test]
    fn a_damaged_key_is_an_error_not_a_new_key() {
        let store = MemoryStore::default();
        store.set(&store_key_name("@ana:example.org"), b"short").unwrap();
        assert!(matches!(store_key(&store, "@ana:example.org"), Err(KeychainError::Corrupt(_))));
    }

    #[test]
    fn debug_never_shows_the_key() {
        let key = store_key(&MemoryStore::default(), "@ana:example.org").unwrap();
        let shown = format!("{key:?}");
        assert_eq!(shown, "StoreKey(<redacted>)");
    }

    #[test]
    fn deleting_a_missing_secret_is_fine() {
        assert!(MemoryStore::default().delete("nothing").is_ok());
    }
}

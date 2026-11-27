//! The unlock passphrase, for systems without a working keychain (ADR 0004).
//!
//! A [`PassphraseVault`] keeps the same secrets the keychain would (store keys and sessions) in one file, encrypted
//! with XChaCha20-Poly1305 under a key derived from the user's passphrase with Argon2id. Nothing in the file is
//! readable without the passphrase: the header holds only the Argon2 parameters and the salt, and it is
//! authenticated with the secrets, so changing it makes the file fail to open rather than weakening the key.
//!
//! The passphrase is never stored. A wrong one is reported as [`VaultError::WrongPassphrase`] and changes nothing
//! on disk; a vault is never created over an existing one. The file is replaced whole (written next to it, then
//! renamed), so a crash leaves the old or the new version, never half of one. Writing keeps to the same limits
//! reading enforces (1 MiB, 4096 secrets), so the vault never writes a file it would refuse to open.
//!
//! A vault in memory is the only copy of what it holds that this process trusts: it writes its whole map, not a
//! merge with the file. Two processes with the same vault open would lose each other's changes, so the app holds
//! an [`crate::InstanceLock`] on the data folder for as long as it runs.
//!
//! File layout, all integers little-endian:
//!
//! | bytes | what |
//! |---|---|
//! | 8 | `PCVAULT1` |
//! | 4 + 4 + 4 | Argon2id memory (KiB), iterations, lanes |
//! | 16 | salt |
//! | 24 | nonce, new on every write |
//! | rest | the secrets, encrypted; the header above is the associated data |

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::{KeychainError, SecretStore};

const MAGIC: &[u8; 8] = b"PCVAULT1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const HEADER_LEN: usize = MAGIC.len() + 12 + SALT_LEN;
const KEY_LEN: usize = 32;
/// XChaCha20-Poly1305's authentication tag, added to the ciphertext.
const TAG_LEN: usize = 16;
/// A vault holds a few keys and sessions; anything much bigger is not one of ours.
const MAX_FILE: u64 = 1024 * 1024;
const MAX_ENTRIES: usize = 4096;

/// Argon2id's cost settings, as stored in a vault's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory in KiB.
    pub memory_kib: u32,
    pub iterations: u32,
    pub lanes: u32,
}

impl KdfParams {
    /// What new vaults use: 64 MiB, three passes, one lane. Well above OWASP's minimum for Argon2id, and about a
    /// second on a laptop, once per unlock.
    pub const DEFAULT: Self = Self { memory_kib: 64 * 1024, iterations: 3, lanes: 1 };

    /// Whether the settings are in the range PingCraft writes. The header comes from a file, so it is hostile
    /// input: a huge memory cost would exhaust memory, a tiny one would make guessing cheap.
    fn is_sane(self) -> bool {
        (8 * 1024..=1024 * 1024).contains(&self.memory_kib) && (1..=16).contains(&self.iterations) && (1..=8).contains(&self.lanes)
    }
}

/// Why a vault could not be created, opened or written. Never contains a secret or the passphrase.
#[derive(Debug)]
pub enum VaultError {
    /// The passphrase is empty.
    EmptyPassphrase,
    /// There is no vault at that path yet.
    NotFound,
    /// A vault is already there; creating another would lose what it holds.
    AlreadyExists,
    /// The passphrase does not open this vault (or the file was changed).
    WrongPassphrase,
    /// The file is not a vault PingCraft can read.
    Corrupt(&'static str),
    /// Reading or writing the file failed.
    Io(String),
    /// The secrets would not fit in a vault (1 MiB, 4096 secrets); nothing was written.
    TooLarge,
    /// The key could not be derived or the system had no random numbers.
    Crypto(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPassphrase => f.write_str("the passphrase is empty"),
            Self::NotFound => f.write_str("there is no PingCraft vault there"),
            Self::AlreadyExists => f.write_str("a PingCraft vault is already there"),
            Self::WrongPassphrase => f.write_str("that passphrase does not unlock PingCraft"),
            Self::Corrupt(what) => write!(f, "the PingCraft vault is damaged: {what}"),
            Self::Io(why) => write!(f, "the PingCraft vault could not be read or written: {why}"),
            Self::TooLarge => f.write_str("the PingCraft vault has no room for more secrets"),
            Self::Crypto(why) => write!(f, "the PingCraft vault could not be encrypted: {why}"),
        }
    }
}

impl std::error::Error for VaultError {}

type Secrets = BTreeMap<String, Zeroizing<Vec<u8>>>;

/// Secrets in a file, encrypted under a key derived from the unlock passphrase.
pub struct PassphraseVault {
    path: PathBuf,
    params: KdfParams,
    /// The salt and the key derived with it; both change when the passphrase does.
    keying: Mutex<Keying>,
    secrets: Mutex<Secrets>,
}

/// A salt and the key derived from the passphrase with it.
struct Keying {
    salt: [u8; SALT_LEN],
    key: Zeroizing<[u8; KEY_LEN]>,
}

impl std::fmt::Debug for PassphraseVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassphraseVault").field("path", &self.path).field("params", &self.params).finish_non_exhaustive()
    }
}

impl PassphraseVault {
    /// Whether a vault exists at `path`, so the app knows whether to ask for a new passphrase or the existing one.
    pub fn exists(path: &Path) -> bool {
        path.exists()
    }

    /// Creates an empty vault, with a new random salt. Fails if one is already there.
    pub fn create(path: &Path, passphrase: &str) -> Result<Self, VaultError> {
        Self::create_with(path, passphrase, KdfParams::DEFAULT)
    }

    /// [`PassphraseVault::create`] with other Argon2 settings (tests use cheap ones). They must be in the range
    /// [`PassphraseVault::unlock`] accepts.
    pub fn create_with(path: &Path, passphrase: &str, params: KdfParams) -> Result<Self, VaultError> {
        if passphrase.is_empty() {
            return Err(VaultError::EmptyPassphrase);
        }
        if !params.is_sane() {
            return Err(VaultError::Crypto("Argon2 settings out of range".into()));
        }
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let key = derive(passphrase, &salt, params)?;
        let vault = Self { path: path.to_owned(), params, keying: Mutex::new(Keying { salt, key }), secrets: Mutex::new(Secrets::new()) };
        vault.save(&Secrets::new())?;
        Ok(vault)
    }

    /// Opens the vault at `path` with the passphrase.
    pub fn unlock(path: &Path, passphrase: &str) -> Result<Self, VaultError> {
        if passphrase.is_empty() {
            return Err(VaultError::EmptyPassphrase);
        }
        let file = read_bounded(path)?;
        let (header, rest) = file.split_at_checked(HEADER_LEN).ok_or(VaultError::Corrupt("too short"))?;
        let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN).ok_or(VaultError::Corrupt("too short"))?;
        let (params, salt) = parse_header(header)?;
        let key = derive(passphrase, &salt, params)?;
        let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let plain = cipher
            .decrypt(<&XNonce>::try_from(nonce).map_err(|_| VaultError::Corrupt("nonce"))?, Payload { msg: ciphertext, aad: header })
            .map(Zeroizing::new)
            .map_err(|_| VaultError::WrongPassphrase)?;
        let secrets = decode(&plain)?;
        Ok(Self { path: path.to_owned(), params, keying: Mutex::new(Keying { salt, key }), secrets: Mutex::new(secrets) })
    }

    /// Changes the passphrase: checks `current` against the one the vault was opened with, then writes the same
    /// secrets under a key derived from `new` with a new salt. On any failure nothing changes, on disk or in memory:
    /// the new key is used only once the file holding it is written.
    pub fn change_passphrase(&self, current: &str, new: &str) -> Result<(), VaultError> {
        if current.is_empty() || new.is_empty() {
            return Err(VaultError::EmptyPassphrase);
        }
        let secrets = self.secrets.lock().map_err(|_| VaultError::Crypto("vault poisoned".into()))?;
        let mut keying = self.keying.lock().map_err(|_| VaultError::Crypto("vault poisoned".into()))?;
        let check = derive(current, &keying.salt, self.params)?;
        // Compared without stopping at the first difference.
        if check.iter().zip(keying.key.iter()).fold(0u8, |d, (a, b)| d | (a ^ b)) != 0 {
            return Err(VaultError::WrongPassphrase);
        }
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let next = Keying { salt, key: derive(new, &salt, self.params)? };
        self.save_with(&secrets, &next)?;
        *keying = next;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Secrets>, KeychainError> {
        self.secrets.lock().map_err(|_| KeychainError::Unavailable("vault poisoned".into()))
    }

    fn header(&self, salt: &[u8; SALT_LEN]) -> [u8; HEADER_LEN] {
        let mut h = [0u8; HEADER_LEN];
        h[..8].copy_from_slice(MAGIC);
        h[8..12].copy_from_slice(&self.params.memory_kib.to_le_bytes());
        h[12..16].copy_from_slice(&self.params.iterations.to_le_bytes());
        h[16..20].copy_from_slice(&self.params.lanes.to_le_bytes());
        h[20..].copy_from_slice(salt);
        h
    }

    /// Encrypts `secrets` with a new nonce and replaces the file. Refuses, changing nothing, when the file would be
    /// one [`PassphraseVault::unlock`] does not open.
    fn save(&self, secrets: &Secrets) -> Result<(), VaultError> {
        let keying = self.keying.lock().map_err(|_| VaultError::Crypto("vault poisoned".into()))?;
        self.save_with(secrets, &keying)
    }

    /// [`Self::save`] under `keying`.
    fn save_with(&self, secrets: &Secrets, keying: &Keying) -> Result<(), VaultError> {
        if secrets.len() > MAX_ENTRIES {
            return Err(VaultError::TooLarge);
        }
        // Counted before anything is copied, so an oversized secret is refused without a second copy of it.
        let plain_len = secrets.iter().fold(4usize, |n, (name, secret)| n.saturating_add(8).saturating_add(name.len()).saturating_add(secret.len()));
        if plain_len.saturating_add(HEADER_LEN + NONCE_LEN + TAG_LEN) as u64 > MAX_FILE {
            return Err(VaultError::TooLarge);
        }
        let plain = encode(secrets);
        let header = self.header(&keying.salt);
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let cipher = XChaCha20Poly1305::new_from_slice(keying.key.as_ref()).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let ciphertext =
            cipher.encrypt(&XNonce::from(nonce), Payload { msg: &plain, aad: &header }).map_err(|e| VaultError::Crypto(e.to_string()))?;
        let mut file = Vec::with_capacity(HEADER_LEN + NONCE_LEN + ciphertext.len());
        file.extend_from_slice(&header);
        file.extend_from_slice(&nonce);
        file.extend_from_slice(&ciphertext);
        write_atomically(&self.path, &file)
    }
}

impl SecretStore for PassphraseVault {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, KeychainError> {
        Ok(self.lock()?.get(name).cloned())
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<(), KeychainError> {
        let mut secrets = self.lock()?;
        let mut next = secrets.clone();
        next.insert(name.to_owned(), Zeroizing::new(secret.to_vec()));
        self.save(&next).map_err(keychain_error)?;
        *secrets = next;
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), KeychainError> {
        let mut secrets = self.lock()?;
        if !secrets.contains_key(name) {
            return Ok(());
        }
        let mut next = secrets.clone();
        next.remove(name);
        self.save(&next).map_err(keychain_error)?;
        *secrets = next;
        Ok(())
    }
}

fn keychain_error(e: VaultError) -> KeychainError {
    match e {
        VaultError::TooLarge => KeychainError::TooLarge,
        other => KeychainError::Unavailable(other.to_string()),
    }
}

fn derive(passphrase: &str, salt: &[u8], params: KdfParams) -> Result<Zeroizing<[u8; KEY_LEN]>, VaultError> {
    let argon_params =
        argon2::Params::new(params.memory_kib, params.iterations, params.lanes, Some(KEY_LEN)).map_err(|e| VaultError::Crypto(e.to_string()))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, argon_params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon.hash_password_into(passphrase.as_bytes(), salt, key.as_mut()).map_err(|e| VaultError::Crypto(e.to_string()))?;
    Ok(key)
}

fn parse_header(header: &[u8]) -> Result<(KdfParams, [u8; SALT_LEN]), VaultError> {
    if header.get(..8) != Some(MAGIC.as_slice()) {
        return Err(VaultError::Corrupt("not a PingCraft vault"));
    }
    let word = |at: usize| header.get(at..at + 4).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes).ok_or(VaultError::Corrupt("header"));
    let params = KdfParams { memory_kib: word(8)?, iterations: word(12)?, lanes: word(16)? };
    if !params.is_sane() {
        return Err(VaultError::Corrupt("Argon2 settings out of range"));
    }
    let salt = header.get(20..HEADER_LEN).and_then(|s| s.try_into().ok()).ok_or(VaultError::Corrupt("header"))?;
    Ok((params, salt))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, VaultError> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(VaultError::NotFound),
        Err(e) => return Err(VaultError::Io(e.to_string())),
    };
    if meta.len() > MAX_FILE {
        return Err(VaultError::Corrupt("too large"));
    }
    std::fs::read(path).map_err(|e| VaultError::Io(e.to_string()))
}

/// Count, then for each secret: name length, name, secret length, secret (lengths as u32).
fn encode(secrets: &Secrets) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::new());
    let len = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes();
    out.extend_from_slice(&len(secrets.len()));
    for (name, secret) in secrets {
        out.extend_from_slice(&len(name.len()));
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&len(secret.len()));
        out.extend_from_slice(secret);
    }
    out
}

fn decode(mut bytes: &[u8]) -> Result<Secrets, VaultError> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Result<&'a [u8], VaultError> {
        let (head, rest) = bytes.split_at_checked(n).ok_or(VaultError::Corrupt("contents"))?;
        *bytes = rest;
        Ok(head)
    }
    fn number(bytes: &mut &[u8]) -> Result<usize, VaultError> {
        let raw: [u8; 4] = take(bytes, 4)?.try_into().map_err(|_| VaultError::Corrupt("contents"))?;
        usize::try_from(u32::from_le_bytes(raw)).map_err(|_| VaultError::Corrupt("contents"))
    }
    let count = number(&mut bytes)?;
    if count > MAX_ENTRIES {
        return Err(VaultError::Corrupt("too many entries"));
    }
    let mut secrets = Secrets::new();
    for _ in 0..count {
        let n = number(&mut bytes)?;
        let name = std::str::from_utf8(take(&mut bytes, n)?).map_err(|_| VaultError::Corrupt("entry name"))?.to_owned();
        let n = number(&mut bytes)?;
        let secret = Zeroizing::new(take(&mut bytes, n)?.to_vec());
        secrets.insert(name, secret);
    }
    if !bytes.is_empty() {
        return Err(VaultError::Corrupt("trailing bytes"));
    }
    Ok(secrets)
}

/// Writes a new version next to `path`, readable by the user only, then renames it over the old one.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let io = |e: std::io::Error| VaultError::Io(e.to_string());
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut tmp_name = path.file_name().map(|n| n.to_os_string()).ok_or_else(|| VaultError::Io("vault path has no file name".into()))?;
    tmp_name.push(".new");
    let tmp = dir.join(tmp_name);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)?;
    drop(file);
    std::fs::rename(&tmp, path).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::{KdfParams, PassphraseVault, VaultError};
    use crate::{KeychainError, SecretStore, store_key};
    use zeroize::Zeroizing;

    /// Cheap settings, so the tests are quick; the lowest the vault accepts.
    const CHEAP: KdfParams = KdfParams { memory_kib: 8 * 1024, iterations: 1, lanes: 1 };

    #[test]
    fn secrets_come_back_after_unlocking_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        let vault = PassphraseVault::create_with(&path, "correct horse", CHEAP).unwrap();
        let key = store_key(&vault, "profile").unwrap();
        vault.set("session:profile", b"{\"token\":\"x\"}").unwrap();
        drop(vault);

        let again = PassphraseVault::unlock(&path, "correct horse").unwrap();
        assert_eq!(store_key(&again, "profile").unwrap().bytes(), key.bytes());
        assert_eq!(again.get("session:profile").unwrap().unwrap().as_slice(), b"{\"token\":\"x\"}");
        again.delete("session:profile").unwrap();
        let third = PassphraseVault::unlock(&path, "correct horse").unwrap();
        assert!(third.get("session:profile").unwrap().is_none());
    }

    /// After a change, the new passphrase opens the vault with the same secrets, the old one no longer does, and the
    /// open vault goes on writing under the new one. A wrong current passphrase changes nothing.
    #[test]
    fn the_passphrase_changes_and_the_secrets_stay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        let vault = PassphraseVault::create_with(&path, "old words", CHEAP).unwrap();
        vault.set("a", b"secret").unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(vault.change_passphrase("not it", "new words"), Err(VaultError::WrongPassphrase)));
        assert!(matches!(vault.change_passphrase("old words", ""), Err(VaultError::EmptyPassphrase)));
        assert_eq!(std::fs::read(&path).unwrap(), before, "a refused change writes nothing");
        vault.change_passphrase("old words", "new words").unwrap();
        vault.set("b", b"later").unwrap();
        drop(vault);
        assert!(matches!(PassphraseVault::unlock(&path, "old words"), Err(VaultError::WrongPassphrase)));
        let again = PassphraseVault::unlock(&path, "new words").unwrap();
        assert_eq!(again.get("a").unwrap().unwrap().as_slice(), b"secret");
        assert_eq!(again.get("b").unwrap().unwrap().as_slice(), b"later", "written under the new passphrase");
    }

    #[test]
    fn a_wrong_passphrase_is_its_own_error_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        let vault = PassphraseVault::create_with(&path, "right", CHEAP).unwrap();
        vault.set("a", b"secret").unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(PassphraseVault::unlock(&path, "wrong"), Err(VaultError::WrongPassphrase)));
        assert!(matches!(PassphraseVault::create_with(&path, "wrong", CHEAP), Err(VaultError::AlreadyExists)));
        assert_eq!(std::fs::read(&path).unwrap(), before, "the vault is untouched");
        assert!(matches!(PassphraseVault::unlock(&path, ""), Err(VaultError::EmptyPassphrase)));
    }

    #[test]
    fn nothing_secret_is_in_the_file_in_the_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        let vault = PassphraseVault::create_with(&path, "a passphrase", CHEAP).unwrap();
        vault.set("session:abc", b"syt_access_token_value").unwrap();
        let file = std::fs::read(&path).unwrap();
        let contains = |needle: &[u8]| file.windows(needle.len()).any(|w| w == needle);
        assert!(!contains(b"syt_access_token_value"));
        assert!(!contains(b"session:abc"));
        assert!(!contains(b"a passphrase"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn a_changed_header_or_body_does_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        PassphraseVault::create_with(&path, "p", CHEAP).unwrap().set("a", b"b").unwrap();
        let good = std::fs::read(&path).unwrap();

        // A flipped salt byte: authenticated, so the key is wrong.
        let mut bad = good.clone();
        bad[25] ^= 1;
        std::fs::write(&path, &bad).unwrap();
        assert!(matches!(PassphraseVault::unlock(&path, "p"), Err(VaultError::WrongPassphrase)));

        // Absurd Argon2 settings are refused before any work is done.
        let mut bad = good.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, &bad).unwrap();
        assert!(matches!(PassphraseVault::unlock(&path, "p"), Err(VaultError::Corrupt(_))));

        std::fs::write(&path, b"PCVAULT1").unwrap();
        assert!(matches!(PassphraseVault::unlock(&path, "p"), Err(VaultError::Corrupt(_))));
        std::fs::write(&path, b"something else entirely, long enough to have a header and a nonce...").unwrap();
        assert!(matches!(PassphraseVault::unlock(&path, "p"), Err(VaultError::Corrupt(_))));
        assert!(matches!(PassphraseVault::unlock(&dir.path().join("missing"), "p"), Err(VaultError::NotFound)));
    }

    #[test]
    fn writing_keeps_to_the_limits_reading_enforces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault");
        let vault = PassphraseVault::create_with(&path, "p", CHEAP).unwrap();
        vault.set("a", b"kept").unwrap();
        let before = std::fs::read(&path).unwrap();

        // A secret that would make the file bigger than a vault may be.
        let big = vec![7u8; super::MAX_FILE as usize];
        assert!(matches!(vault.set("big", &big), Err(KeychainError::TooLarge)));
        assert!(vault.get("big").unwrap().is_none(), "nothing changed in memory");

        // One secret more than a vault may hold. They are put in memory directly: writing 4096 files is slow.
        {
            let mut secrets = vault.secrets.lock().unwrap();
            for i in 1..super::MAX_ENTRIES {
                secrets.insert(format!("n{i}"), Zeroizing::new(vec![1]));
            }
        }
        assert!(matches!(vault.set("one more", b"x"), Err(KeychainError::TooLarge)));

        assert_eq!(std::fs::read(&path).unwrap(), before, "the file is untouched");
        let again = PassphraseVault::unlock(&path, "p").unwrap();
        assert_eq!(again.get("a").unwrap().unwrap().as_slice(), b"kept");
    }

    #[test]
    fn debug_shows_no_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let vault = PassphraseVault::create_with(&dir.path().join("vault"), "hunter2", CHEAP).unwrap();
        vault.set("session:x", b"token-value").unwrap();
        let shown = format!("{vault:?}");
        assert!(!shown.contains("hunter2") && !shown.contains("token-value") && !shown.contains("session:x"), "{shown}");
        let err = format!("{:?} {}", VaultError::WrongPassphrase, VaultError::WrongPassphrase);
        assert!(!err.contains("hunter2"));
    }
}

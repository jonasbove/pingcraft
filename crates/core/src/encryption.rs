//! End-to-end encryption as the interface shows it: the recovery key, and verifying another device by comparing
//! symbols (ADR 0004). The Matrix adapter fills these in; the interface renders them and sends back the user's
//! answers.

use zeroize::Zeroizing;

/// Whether this device can read the user's encrypted history from the server's key backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecoveryStatus {
    /// Not known yet (the first sync has not finished).
    #[default]
    Unknown,
    /// No recovery key has been set up for the account. The first sign-in sets one up.
    Off,
    /// Set up, and this device has everything it needs.
    On,
    /// Set up on the account, but this device has not been given the recovery key yet: the user should type it in
    /// (or verify this device from another one).
    NeedsKey,
}

/// Why a new recovery key was not made, when it is something the user can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryRefusal {
    /// The server wants the account password first.
    NeedsPassword,
    /// The server did not take the account password given.
    WrongPassword,
    /// The server wants another way of proving it is the user, which PingCraft cannot do yet.
    NotSupported,
}

/// The account's recovery key, as the user saves it or types it in. Wiped from memory when dropped, and never
/// shown by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKey(Zeroizing<String>);

impl RecoveryKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(Zeroizing::new(key.into()))
    }

    /// The key as the user types it: groups of four characters, separated by spaces.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The key in groups of four, for showing in rows. Spaces the user typed are ignored.
    pub fn groups(&self) -> Vec<String> {
        let compact: Vec<char> = self.0.chars().filter(|c| !c.is_whitespace()).collect();
        compact.chunks(4).map(|c| c.iter().collect()).collect()
    }
}

impl std::fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryKey(<redacted>)")
    }
}

/// One of the seven symbols two devices compare. `index` is its place in the Matrix specification's SAS emoji
/// table (0 to 63), so the interface can show the name in the user's language; `name` is the table's English
/// name, as a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SasSymbol {
    pub index: u8,
    pub symbol: String,
    pub name: String,
}

/// Where a verification between two devices is. The interface shows one sheet per state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verification {
    /// Another device asked to verify this one; the user can accept or decline.
    Incoming { device: String },
    /// Waiting for the other device (to accept, or to show its symbols).
    Waiting,
    /// Both devices show these symbols; the user says whether they match.
    Compare(Vec<SasSymbol>),
    /// The user said they match; waiting for the other device to say so too.
    Confirmed,
    /// Verified: the devices trust each other.
    Done,
    /// It ended without verifying: declined, timed out, or the symbols did not match.
    Cancelled(CancelReason),
}

/// Why a verification ended, in terms the interface can put in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelReason {
    /// Someone said the symbols do not match. Nothing was trusted.
    Mismatch,
    /// One of the two devices cancelled or declined.
    Cancelled,
    /// The other device did not answer in time.
    TimedOut,
    /// Anything else, in the adapter's words, for the log.
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::RecoveryKey;

    #[test]
    fn the_recovery_key_never_shows_in_debug() {
        let key = RecoveryKey::new("EsT1 abcd efgh");
        assert_eq!(format!("{key:?}"), "RecoveryKey(<redacted>)");
        assert_eq!(key.groups(), ["EsT1", "abcd", "efgh"]);
    }
}

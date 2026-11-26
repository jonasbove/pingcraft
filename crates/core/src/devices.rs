//! The user's signed-in devices, as Settings lists them: this one, which the user can rename, and the others, which
//! the user can verify or sign out. A device is a Matrix session; PingCraft calls it a device, as GLOSSARY.md does.

use crate::Timestamp;

/// A device's ID, as the server knows it. Not a secret, but it says nothing a person needs to read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub String);

/// One of the user's signed-in devices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    /// The name other devices see; empty when it has none.
    pub name: String,
    /// When the server last heard from it, if it says.
    pub last_seen: Option<Timestamp>,
    /// Whether the user's own cross-signing vouches for it.
    pub verified: bool,
    /// Whether it is the device PingCraft runs on.
    pub this: bool,
}

/// Why another device was not signed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignOutProblem {
    /// The server refused the account password.
    WrongPassword,
    /// The server wants another way of proving it is the user (single sign-on, say), which PingCraft cannot do yet.
    NotSupported,
    /// Anything else, in the adapter's words.
    Other(String),
}

/// The longest device name PingCraft sends. The Matrix specification sets no limit; this keeps a typo'd paste from
/// going to the server.
pub const MAX_NAME: usize = 100;

/// A name as typed, made ready to send: trimmed, cut to [`MAX_NAME`] characters. `None` when nothing is left.
pub fn clean_name(typed: &str) -> Option<String> {
    let name: String = typed.trim().chars().take(MAX_NAME).collect();
    let name = name.trim_end().to_owned();
    (!name.is_empty()).then_some(name)
}

/// The devices in the order Settings lists them: this one first, then the others, most recently seen first.
pub fn sorted(mut devices: Vec<Device>) -> Vec<Device> {
    devices.sort_by(|a, b| b.this.cmp(&a.this).then(b.last_seen.cmp(&a.last_seen)).then(a.id.cmp(&b.id)));
    devices
}

#[cfg(test)]
mod tests {
    use super::{Device, DeviceId, MAX_NAME, clean_name, sorted};
    use crate::Timestamp;

    #[test]
    fn a_name_is_trimmed_and_capped_and_an_empty_one_is_none() {
        assert_eq!(clean_name("  Laptop \n").as_deref(), Some("Laptop"));
        assert_eq!(clean_name("   "), None);
        let long = "é".repeat(MAX_NAME + 20);
        assert_eq!(clean_name(&long).map(|n| n.chars().count()), Some(MAX_NAME));
    }

    #[test]
    fn this_device_comes_first_then_the_most_recently_seen() {
        let d = |id: &str, seen: Option<u64>, this: bool| Device {
            id: DeviceId(id.into()),
            name: String::new(),
            last_seen: seen.map(Timestamp),
            verified: false,
            this,
        };
        let order: Vec<String> = sorted(vec![d("old", Some(1), false), d("never", None, false), d("me", Some(0), true), d("new", Some(9), false)])
            .into_iter()
            .map(|d| d.id.0)
            .collect();
        assert_eq!(order, ["me", "new", "old", "never"]);
    }
}

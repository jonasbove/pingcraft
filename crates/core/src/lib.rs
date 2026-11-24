//! PingCraft's domain model, in the words of `GLOSSARY.md`.
//!
//! Layer: lowest. Nothing here depends on the Matrix SDK or on egui (ADR 0009), so the interface can be
//! tested without a server and SDK upgrades stay in `pingcraft-matrix`.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

/// A chat service a user can connect, such as Telegram. Matrix itself is also a network.
///
/// Networks are open-ended (a bridge provider can offer new ones), so this is an identifier, not an enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Network(String);

impl Network {
    /// The network a chat belongs to when no bridge claims it.
    pub fn matrix() -> Self {
        Self("matrix".to_owned())
    }

    /// A network by the identifier its bridge reports (for mautrix: `protocol.id` in the bridge-info state).
    pub fn from_id(id: &str) -> Self {
        Self(id.to_ascii_lowercase())
    }

    pub fn id(&self) -> &str {
        &self.0
    }

    pub fn is_matrix(&self) -> bool {
        self.0 == "matrix"
    }
}

#[cfg(test)]
mod tests {
    use super::Network;

    #[test]
    fn network_ids_are_case_insensitive() {
        assert_eq!(Network::from_id("Telegram"), Network::from_id("telegram"));
        assert!(Network::matrix().is_matrix());
        assert!(!Network::from_id("telegram").is_matrix());
    }
}

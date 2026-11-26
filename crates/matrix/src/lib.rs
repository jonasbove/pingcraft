//! PingCraft's adapter onto the Matrix SDK. Everything Matrix-specific stays in this crate; the interface sees
//! only `pingcraft-core` types (ADR 0009).
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub use pingcraft_core as core;

// Before the modules, so they can use it.
/// The fixed prefix of PingCraft's own data on the server (account data and any event types of its own), the same
/// on every server, whoever runs it: a reverse domain PingCraft controls (GLOSSARY.md, "Namespace"). Each name is
/// this, a dot and the name, written out with [`namespaced!`] so it stays a `&'static str`.
pub const NAMESPACE: &str = "io.github.jonasbove.pingcraft";

/// A name in PingCraft's [`NAMESPACE`], as a string literal.
macro_rules! namespaced {
    ($name:literal) => {
        concat!("io.github.jonasbove.pingcraft.", $name)
    };
}
pub(crate) use namespaced;

// Native only, like the engine that uses it (its timeouts use tokio's timer).
#[cfg(not(target_arch = "wasm32"))]
mod account_data;
mod bridge_info;
mod chat_settings;
#[cfg(not(target_arch = "wasm32"))]
mod connect;
pub mod connection;
pub mod devices;
#[cfg(not(target_arch = "wasm32"))]
pub mod emoji_settings;
pub mod encryption;
#[cfg(not(target_arch = "wasm32"))]
pub mod engine;
pub mod inbox;
#[cfg(not(target_arch = "wasm32"))]
pub mod mautrix;
// Native only for now, like the engine that uses it (its timing uses `std::time::Instant`).
#[cfg(not(target_arch = "wasm32"))]
pub mod network_alerts;
#[cfg(not(target_arch = "wasm32"))]
pub mod recent_emoji;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
pub mod timeline;

pub use connection::{Connection, SyncError};
pub use inbox::InboxFeed;
pub use timeline::{ChatTimeline, ChatUpdate, TimelineError};

/// The SDK version this adapter is written against, for the about screen and bug reports.
pub const MATRIX_SDK_VERSION: &str = "0.19.1";

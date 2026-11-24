//! PingCraft's adapter onto the Matrix SDK. Everything Matrix-specific stays in this crate; the interface sees
//! only `pingcraft-core` types (ADR 0009).
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub use pingcraft_core as core;

mod bridge_info;
#[cfg(not(target_arch = "wasm32"))]
mod connect;
pub mod connection;
pub mod encryption;
#[cfg(not(target_arch = "wasm32"))]
pub mod engine;
pub mod inbox;
#[cfg(not(target_arch = "wasm32"))]
pub mod mautrix;
#[cfg(not(target_arch = "wasm32"))]
pub mod session;
pub mod timeline;

pub use connection::{Connection, SyncError};
pub use inbox::InboxFeed;
pub use timeline::{ChatTimeline, TimelineError};

/// The SDK version this adapter is written against, for the about screen and bug reports.
pub const MATRIX_SDK_VERSION: &str = "0.19.1";

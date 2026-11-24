//! PingCraft's interface. It sees only `pingcraft-core` types, never Matrix SDK ones (ADR 0009).
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub use pingcraft_core as core;

//! The PingCraft web build. Milestone 4 fills it in; for now it only keeps the web crates checked for wasm.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

pub use pingcraft_ui_egui as ui;

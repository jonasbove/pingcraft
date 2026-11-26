//! Finds the pinned colour emoji font, checks its hash, and has the crate embed it (`cfg(color_emoji)`).
//!
//! The build never downloads anything: `cargo xtask fonts` fetches the pinned file into the fonts folder
//! (`CRAFT_FONTS_DIR`, or `.cache/craft-fonts` in the workspace). Without it, emoji are drawn with egui's
//! monochrome emoji font, unless `CRAFT_FONTS_REQUIRED=1`, which makes a missing font an error (`cargo xtask ci`
//! and the packages set it). A file that is not the pinned one is always an error. The web build embeds no colour
//! font: it fetches the pinned file after the page opens (`emoji::web`, PLAN.md, "Messages and chats" 1).

use std::path::PathBuf;

use sha2::{Digest, Sha256};

#[allow(dead_code)]
mod pin {
    include!("font_pin.rs");
}

fn main() {
    println!("cargo::rustc-check-cfg=cfg(color_emoji)");
    println!("cargo::rerun-if-changed=font_pin.rs");
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_DIR");
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_REQUIRED");
    if std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|a| a == "wasm32") {
        return;
    }
    let required = std::env::var("CRAFT_FONTS_REQUIRED").is_ok_and(|v| v == "1");
    let dir = match std::env::var_os("CRAFT_FONTS_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()).join("../..").join(pin::DEFAULT_FONTS_DIR),
    };
    let path = dir.join(pin::EMOJI_FONT_PATH);
    println!("cargo::rerun-if-changed={}", path.display());
    match std::fs::read(&path) {
        Ok(bytes) => {
            let hash = format!("{:x}", Sha256::digest(&bytes));
            if hash != pin::EMOJI_FONT_SHA256 {
                println!(
                    "cargo::error={} is not the pinned Noto Color Emoji (SHA-256 {hash}, expected {}). Delete it and run `cargo xtask fonts`.",
                    path.display(),
                    pin::EMOJI_FONT_SHA256
                );
                return;
            }
            println!("cargo::rustc-cfg=color_emoji");
            println!("cargo::rustc-env=PINGCRAFT_EMOJI_FONT={}", path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => {
            println!(
                "cargo::warning=No colour emoji font at {}: emoji are drawn in monochrome. `cargo xtask fonts` fetches the pinned one.",
                path.display()
            );
        }
        Err(e) => println!("cargo::error=The colour emoji font {}: {e}. `cargo xtask fonts` fetches the pinned one.", path.display()),
    }
}

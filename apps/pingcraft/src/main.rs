//! The PingCraft desktop app: the egui interface over the Matrix engine.
//!
//! Its data (the encrypted stores, and the vault where there is no keychain) is in the platform's data folder,
//! or in `PINGCRAFT_DATA_DIR` when that is set. `PINGCRAFT_NO_KEYCHAIN=1` makes it use the unlock passphrase even
//! where a keychain works, to try that path.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
// A Windows GUI program: no console window opens with it (packaging/windows/package.ps1 checks this).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use pingcraft_keychain::KdfParams;
use pingcraft_matrix::engine::{Engine, Secrets};
use pingcraft_ui_egui::PingCraftApp;

/// The folder name under the platform's data folder.
const APP_ID: &str = "pingcraft";

struct Window {
    app: PingCraftApp,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("PINGCRAFT_DATA_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => Some(PathBuf::from(dir)),
        None => craft_settings::paths::data_dir(APP_ID),
    }
}

fn options() -> eframe::NativeOptions {
    let viewport = egui::ViewportBuilder::default()
        .with_title("PingCraft")
        // The Wayland app id and the X11 window class, for the desktop entry.
        .with_app_id("io.github.jonasbove.pingcraft")
        .with_inner_size([1200.0, 760.0])
        .with_min_inner_size([900.0, 600.0]);
    let viewport = match eframe::icon_data::from_png_bytes(pingcraft_ui_egui::ICON_PNG) {
        Ok(icon) => viewport.with_icon(Arc::new(icon)),
        Err(e) => {
            eprintln!("pingcraft: no window icon: {e}");
            viewport
        }
    };
    eframe::NativeOptions { viewport, ..Default::default() }
}

fn main() -> ExitCode {
    // Answered without a display, so packaging can check that the binary starts.
    if std::env::args().nth(1).is_some_and(|a| a == "--version" || a == "-V") {
        println!("pingcraft {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let Some(data) = data_dir() else {
        eprintln!("pingcraft: this system has no data folder; set PINGCRAFT_DATA_DIR");
        return ExitCode::FAILURE;
    };
    if let Err(e) = std::fs::create_dir_all(&data) {
        eprintln!("pingcraft: could not make {}: {e}", data.display());
        return ExitCode::FAILURE;
    }
    let secrets =
        if std::env::var_os("PINGCRAFT_NO_KEYCHAIN").is_some_and(|v| v == "1") { Secrets::Passphrase(KdfParams::DEFAULT) } else { Secrets::System };
    pingcraft_ui_egui::i18n::use_system_language();
    let result = eframe::run_native(
        "PingCraft",
        options(),
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let engine = Engine::start(data, secrets, Arc::new(move || ctx.request_repaint()))?;
            Ok(Box::new(Window { app: PingCraftApp::new(Box::new(engine)) }))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pingcraft: could not open the window: {e}");
            ExitCode::FAILURE
        }
    }
}

//! The PingCraft desktop app: the egui interface over the Matrix engine.
//!
//! Its data (the encrypted stores, and the vault where there is no keychain) is in the platform's data folder,
//! or in `PINGCRAFT_DATA_DIR` when that is set. `PINGCRAFT_NO_KEYCHAIN=1` makes it use the unlock passphrase even
//! where a keychain works, to try that path.
//!
//! Its settings on this computer are craft-settings' `config.toml` in the platform's configuration folder, or in
//! `PINGCRAFT_CONFIG_DIR`. For now that is a copy of the emoji skin tone, which the account keeps: the copy is what
//! PingCraft starts with, and moved up once to an account that keeps none.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]
// A Windows GUI program: no console window opens with it (packaging/windows/package.ps1 checks this).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use craft_settings::{Location, SettingsStore};
use serde::{Deserialize, Serialize};

use pingcraft_keychain::KdfParams;
use pingcraft_matrix::engine::{Engine, Secrets};
use pingcraft_ui_egui::{PingCraftApp, SkinTone};

/// The folder name under the platform's data folder.
const APP_ID: &str = "pingcraft";

/// PingCraft's settings on this computer. What belongs to the account (recently used emoji, the skin tone) is on the
/// server.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
struct Prefs {
    emoji: EmojiPrefs,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
struct EmojiPrefs {
    /// The skin tone last used, by [`SkinTone::id`]: a copy of the account's, to start with and for when the server
    /// cannot be reached.
    skin_tone: String,
    /// An account has said which tone it keeps since, so this copy was moved up to it once (or set aside) and is
    /// never moved up again, to this or another account.
    skin_tone_migrated: bool,
}

impl craft_settings::Settings for Prefs {
    const APP_ID: &'static str = APP_ID;
    const SCHEMA_VERSION: u32 = 1;
}

struct Window {
    app: PingCraftApp,
    settings: SettingsStore<Prefs>,
}

/// Keeps a copy of the skin tone in use in the settings, which save it a moment later.
fn keep_skin_tone(tone: SkinTone, migrated: bool, settings: &mut SettingsStore<Prefs>) {
    let kept = &settings.get().emoji;
    if SkinTone::from_id(&kept.skin_tone) != tone || kept.skin_tone_migrated != migrated {
        settings.update(|p| {
            p.emoji.skin_tone = tone.id().to_owned();
            p.emoji.skin_tone_migrated = migrated;
        });
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
        keep_skin_tone(self.app.skin_tone(), self.app.skin_tone_migrated(), &mut self.settings);
        self.settings.tick(Instant::now());
    }

    fn on_exit(&mut self) {
        // Before the engine is dropped (with the app, after this), which handles what is queued before it stops.
        self.app.closing();
        if let Err(e) = self.settings.flush() {
            eprintln!("pingcraft: the settings could not be saved: {e}");
        }
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
            let mut app = PingCraftApp::new(Box::new(engine));
            let (settings, _) = SettingsStore::<Prefs>::open(Location::Auto);
            let kept = &settings.get().emoji;
            app.set_local_skin_tone(SkinTone::from_id(&kept.skin_tone), kept.skin_tone_migrated);
            let ctx = cc.egui_ctx.clone();
            app.set_notifier(pingcraft_notify::desktop(Arc::new(move || ctx.request_repaint())));
            Ok(Box::new(Window { app, settings }))
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

#[cfg(test)]
mod tests {
    use craft_settings::SettingsStore;
    use pingcraft_ui_egui::SkinTone;

    use super::{Prefs, keep_skin_tone};

    /// The local copy of the skin tone, and whether it was moved up to the account, are saved and come back on the
    /// next start.
    #[test]
    fn the_skin_tone_is_saved_and_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let (mut settings, _) = SettingsStore::<Prefs>::open_at(dir.path());
        assert_eq!(SkinTone::from_id(&settings.get().emoji.skin_tone), SkinTone::Default, "nothing saved yet");
        assert!(!settings.get().emoji.skin_tone_migrated);
        keep_skin_tone(SkinTone::MediumDark, false, &mut settings);
        settings.flush().unwrap();
        let (mut again, _) = SettingsStore::<Prefs>::open_at(dir.path());
        assert_eq!(again.get().emoji.skin_tone, "medium-dark");
        assert_eq!(SkinTone::from_id(&again.get().emoji.skin_tone), SkinTone::MediumDark);
        assert!(!again.get().emoji.skin_tone_migrated);
        keep_skin_tone(SkinTone::MediumDark, true, &mut again);
        again.flush().unwrap();
        let (third, _) = SettingsStore::<Prefs>::open_at(dir.path());
        assert!(third.get().emoji.skin_tone_migrated, "moved up once, never again");
    }

    /// A settings file from before the account kept the tone has no `skin_tone_migrated`: it reads as not moved up
    /// yet, so its tone moves up once.
    #[test]
    fn an_older_settings_file_has_not_moved_its_tone_up() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(craft_settings::store::CONFIG_FILE), "schema_version = 1\n\n[emoji]\nskin_tone = \"dark\"\n").unwrap();
        let (again, _) = SettingsStore::<Prefs>::open_at(dir.path());
        assert_eq!(again.get().emoji.skin_tone, "dark");
        assert!(!again.get().emoji.skin_tone_migrated);
    }
}

//! craft-ui's runtime, hosted from PingCraft's own eframe `App` through the `craft_ui::shell::embed` hooks (craft-libs
//! docs/craft-framework.md, section 2.2): [`install`] once when the window is created, [`before_input`] in
//! `raw_input_hook`, [`close_path`] last in `logic` and [`before_ui`] first in `ui`. The tests install it with
//! [`install_headless`].
//!
//! The runtime takes Tab, F6 and Escape from egui for the whole window: Tab moves over one order of every focusable
//! control, and an Escape that none of PingCraft's own widgets had focus for arrives as the window's cancel
//! ([`take_window_cancel`]).

use std::sync::{Arc, LazyLock};

use craft_host::target::ShellInfo;
use craft_host::{AppInfo, Host};
use craft_ui::InstallSpec;
use craft_ui::runtime::env::RootEnv;
use craft_ui::runtime::frame::{FrameInput, set_input};
use craft_ui::runtime::keys::{Intent, with_window_intents};
use craft_ui::shell::embed::{self, CloseDecision};
use craft_window::system::{NoWindows, WindowSystem};
use egui::{Context, Key, Modifiers};

/// The app's name and version for the runtime's dialogs and words. PingCraft's words are not craft-i18n's yet
/// (PLAN.md, M2 ticket 9), so the host carries no catalogs and the framework's own words are English.
static HOST: LazyLock<Arc<Host>> =
    LazyLock::new(|| Arc::new(Host::builder(AppInfo { id: "pingcraft", name: "PingCraft", version: env!("CARGO_PKG_VERSION") }).build()));

/// The root values of every frame: craft-libs' defaults (its default theme, the system's light or dark).
fn input() -> FrameInput {
    FrameInput::new(RootEnv::new()).host(Arc::clone(&HOST))
}

fn install_with(ctx: &Context, spec: InstallSpec) -> Result<(), craft_ui::InstallError> {
    match craft_ui::install(ctx, spec) {
        Ok(()) | Err(craft_ui::InstallError::AlreadyInstalled) => {}
        Err(error) => return Err(error),
    }
    set_input(ctx, input());
    Ok(())
}

/// Installs the runtime on the desktop window's context, with the platform's window services and the OS appearance
/// reader. Call it once, when the window is created, before the first frame.
#[cfg(not(target_arch = "wasm32"))]
pub fn install(ctx: &Context, app_id: &str) -> Result<(), craft_ui::InstallError> {
    use craft_host::target::Backend;
    use craft_ui::runtime::env::SystemReader;
    let wake = ctx.clone();
    let spec = InstallSpec::new(ShellInfo::default().backend(Backend::from_env()), Arc::new(craft_window::system::SystemWindows))
        .system(SystemReader::start(move || wake.request_repaint()))
        .app_id(app_id);
    install_with(ctx, spec)
}

/// Installs the runtime with no window services: the web build, and the tests.
pub fn install_headless(ctx: &Context) {
    let windows: Arc<dyn WindowSystem> = Arc::new(NoWindows);
    if let Err(error) = install_with(ctx, InstallSpec::new(ShellInfo::default(), windows)) {
        log::warn!("the UI runtime could not be installed: {error}");
    }
}

/// From `raw_input_hook`, before each frame: the root values and the host.
pub fn before_input(ctx: &Context) {
    embed::before_input(ctx, |_output| input());
}

/// First in `ui`, each pass: hands the runtime the window (eframe's `Frame`) and fits it to its monitor once.
#[cfg(not(target_arch = "wasm32"))]
pub use craft_ui::shell::embed::before_ui;

/// Last in `logic`: the window's close path. PingCraft has nothing to ask before it closes (drafts are kept as it
/// does), so the window closes.
pub fn close_path(ctx: &Context) {
    embed::logic(ctx, || CloseDecision::Close);
}

/// Takes this pass's Escape: egui's, while one of PingCraft's own widgets has focus, else the window's cancel that no
/// craft-ui component took. Whether there was one.
pub fn take_window_cancel(ctx: &Context) -> bool {
    ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) || with_window_intents(ctx, false, |intents| intents.take(Intent::Cancel))
}

//! craft-ui's runtime, hosted from PingCraft's own eframe `App` through the `craft_ui::shell::embed` hooks (craft-libs
//! docs/craft-framework.md, section 2.2): [`install`] once when the window is created, [`before_input`] in
//! `raw_input_hook`, [`close_path`] last in `logic` and [`before_ui`] first in `ui`. The tests install it with
//! [`install_headless`] and run [`before_input`] from an egui plugin's input hook, which egui calls at the same point.
//!
//! [`before_input`] hands the runtime the input the interface asked for at the end of its last frame ([`set_next`]:
//! the theme's mode, PingCraft's commands and their state), and keeps the commands fired in the frame before for the
//! interface to run ([`take_fired`]).
//!
//! The runtime takes Tab, F6 and Escape from egui for the whole window: Tab moves over one order of every focusable
//! control, and a sheet (a craft-ui dialog) keeps Tab inside it and closes on Escape.

use std::sync::{Arc, LazyLock};

use craft_host::target::ShellInfo;
use craft_host::{AppInfo, Host};
use craft_theme::{ModeChoice, ThemeDef, ThemeId};
use craft_ui::InstallSpec;
use craft_ui::runtime::commands::{CommandInput, CommandRef};
use craft_ui::runtime::env::RootEnv;
use craft_ui::runtime::frame::{FrameInput, set_input};
use craft_ui::shell::embed::{self, CloseDecision};
use craft_window::system::{NoWindows, WindowSystem};
use egui::Context;

/// The app's name and version for the runtime's dialogs and words. PingCraft's words are not craft-i18n's yet
/// (PLAN.md, M2 ticket 9), so the host carries no catalogs and the framework's own words are English.
static HOST: LazyLock<Arc<Host>> =
    LazyLock::new(|| Arc::new(Host::builder(AppInfo { id: "pingcraft", name: "PingCraft", version: env!("CARGO_PKG_VERSION") }).build()));

/// The theme a frame resolves from: a built-in by id, or a theme the app holds (PingCraft's own, or the suite
/// library's).
#[derive(Clone)]
pub enum ThemeSource {
    BuiltIn(ThemeId),
    Held(Arc<ThemeDef>),
}

/// The root values of a frame: the theme and the mode the user chose (light, dark, or the system's), and
/// PingCraft's commands with their state.
pub fn input(mode: ModeChoice, theme: ThemeSource, commands: CommandInput) -> FrameInput {
    let root = RootEnv::new().mode(mode);
    let root = match theme {
        ThemeSource::BuiltIn(id) => root.theme(id),
        ThemeSource::Held(def) => root.library_theme(def),
    };
    FrameInput::new(root).host(Arc::clone(&HOST)).commands(commands)
}

/// craft-libs' default theme, following the system.
fn default_input() -> FrameInput {
    input(ModeChoice::System, ThemeSource::BuiltIn(ThemeId::suite_default()), CommandInput::new())
}

fn install_with(ctx: &Context, spec: InstallSpec) -> Result<(), craft_ui::InstallError> {
    match craft_ui::install(ctx, spec) {
        Ok(()) | Err(craft_ui::InstallError::AlreadyInstalled) => {}
        Err(error) => return Err(error),
    }
    set_input(ctx, default_input());
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

/// What the interface asked the next frame's input to be.
#[derive(Clone)]
struct Next {
    mode: ModeChoice,
    theme: ThemeSource,
    commands: CommandInput,
}

/// The commands fired in earlier frames that the interface has not taken yet.
#[derive(Clone, Default)]
struct Fired(Vec<CommandRef>);

fn next_id() -> egui::Id {
    egui::Id::unique("pingcraft-next-input")
}

fn fired_id() -> egui::Id {
    egui::Id::unique("pingcraft-fired")
}

/// From `raw_input_hook`, before each frame: keeps the commands the frame before fired, and hands the runtime the
/// input the interface asked for.
pub fn before_input(ctx: &Context) {
    embed::before_input(ctx, |output| {
        let fired: Vec<CommandRef> = output.fired.into_iter().map(|f| f.command).collect();
        let next = ctx.data_mut(|d| {
            if !fired.is_empty() {
                d.get_temp_mut_or_default::<Fired>(fired_id()).0.extend(fired);
            }
            d.get_temp::<Next>(next_id())
        });
        match next {
            Some(Next { mode, theme, commands }) => input(mode, theme, commands),
            None => default_input(),
        }
    });
}

/// The input the next frame should have: the mode and theme, and PingCraft's commands with their state.
pub fn set_next(ctx: &Context, mode: ModeChoice, theme: ThemeSource, commands: CommandInput) {
    ctx.data_mut(|d| d.insert_temp(next_id(), Next { mode, theme, commands }));
}

/// The commands fired since the last call (a shortcut, a menu row, a command button), oldest first.
pub fn take_fired(ctx: &Context) -> Vec<CommandRef> {
    ctx.data_mut(|d| d.remove_temp::<Fired>(fired_id())).map(|f| f.0).unwrap_or_default()
}

/// First in `ui`, each pass: hands the runtime the window (eframe's `Frame`) and fits it to its monitor once.
#[cfg(not(target_arch = "wasm32"))]
pub use craft_ui::shell::embed::before_ui;

/// Last in `logic`: the window's close path. PingCraft has nothing to ask before it closes (drafts are kept as it
/// does), so the window closes.
pub fn close_path(ctx: &Context) {
    embed::logic(ctx, || CloseDecision::Close);
}

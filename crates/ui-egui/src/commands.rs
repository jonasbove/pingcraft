//! PingCraft's own commands (rule 14): the window's tabs, Settings and what its pages do, registered with craft-ui's
//! command board as an app catalog, so a shortcut, a menu row, a button on a page and (later) the automation interface
//! all run the same command. The app menu (craft-libs `menu`) lists them; craft-menu lays it out for the platform
//! (on Linux and Windows, Settings goes last in Edit as "Preferences…", as in the other Craft apps).
//!
//! The only default shortcut is the Craft apps' Settings key, ⌘, (Ctrl+, off macOS). The others have none until the
//! design gives them one.

use craft_menu::{Item, ItemRole, LayoutOptions, Menu, MenuBar, MenuRole, Node, Target};
use craft_ui::contract::Tracked;
use craft_ui::runtime::commands::{AppCatalog, Binding, CommandDef, CommandRef, CommandState, CommandStates, Scope};

use crate::state::{Appearance, Screen, State};
use crate::tl;

/// One of PingCraft's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCommand {
    /// Shows the inbox.
    Inbox,
    /// Shows the ACCOUNTS tab.
    Accounts,
    /// Shows Settings.
    Settings,
    /// Reconnects now and asks the bridges again.
    Refresh,
    /// Light, dark, or as the system is.
    Appearance(Appearance),
    /// Opens the sheet that changes the unlock passphrase.
    ChangePassphrase,
    /// Opens the sheet that makes a new recovery key.
    NewRecoveryKey,
    /// Opens the sheet that types in the account's recovery key.
    EnterRecoveryKey,
    /// Asks the user's other devices to verify this one.
    VerifyThisDevice,
    /// Opens the sheet that signs this device out.
    SignOut,
}

impl AppCommand {
    /// Every command, in the catalog's order.
    pub const ALL: [Self; 12] = [
        Self::Inbox,
        Self::Accounts,
        Self::Settings,
        Self::Refresh,
        Self::Appearance(Appearance::Light),
        Self::Appearance(Appearance::Dark),
        Self::Appearance(Appearance::System),
        Self::ChangePassphrase,
        Self::NewRecoveryKey,
        Self::EnterRecoveryKey,
        Self::VerifyThisDevice,
        Self::SignOut,
    ];

    /// The id it is registered, bound and stored under. Never change one: a user's own shortcuts name it.
    pub fn id(self) -> &'static str {
        match self {
            Self::Inbox => "view.inbox",
            Self::Accounts => "view.accounts",
            Self::Settings => "app.settings",
            Self::Refresh => "view.refresh",
            Self::Appearance(Appearance::Light) => "appearance.light",
            Self::Appearance(Appearance::Dark) => "appearance.dark",
            Self::Appearance(Appearance::System) => "appearance.system",
            Self::ChangePassphrase => "security.change_passphrase",
            Self::NewRecoveryKey => "security.new_recovery_key",
            Self::EnterRecoveryKey => "security.enter_recovery_key",
            Self::VerifyThisDevice => "security.verify_this_device",
            Self::SignOut => "security.sign_out",
        }
    }

    /// The command a fired reference names, if it is one of PingCraft's.
    pub fn from_ref(command: &CommandRef) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == command.id)
    }

    /// Its name in menus, the keyboard shortcuts list and the command palette.
    pub fn label(self) -> &'static str {
        match self {
            Self::Inbox => tl!("Inbox"),
            Self::Accounts => tl!("Accounts"),
            Self::Settings => tl!("Settings…"),
            Self::Refresh => tl!("Refresh"),
            Self::Appearance(Appearance::Light) => tl!("Light"),
            Self::Appearance(Appearance::Dark) => tl!("Dark"),
            Self::Appearance(Appearance::System) => tl!("Sync with system"),
            Self::ChangePassphrase => tl!("Change passphrase…"),
            Self::NewRecoveryKey => tl!("Create a new recovery key…"),
            Self::EnterRecoveryKey => tl!("Enter recovery key…"),
            Self::VerifyThisDevice => tl!("Verify this device"),
            Self::SignOut => tl!("Sign out…"),
        }
    }

    /// Its default shortcut, in craft-keymap's words.
    fn default_keys(self) -> Option<&'static str> {
        match self {
            Self::Settings => Some("Cmd+,"),
            _ => None,
        }
    }

    /// Whether it can run now. A command that cannot fires nothing and is greyed in the menus.
    pub fn enabled(self, state: &State) -> bool {
        use pingcraft_core::encryption::RecoveryStatus;
        let signed_in = state.screen == Screen::Inbox;
        match self {
            Self::Inbox | Self::Accounts | Self::Settings | Self::Refresh | Self::VerifyThisDevice | Self::SignOut => signed_in,
            Self::Appearance(_) => true,
            Self::ChangePassphrase => signed_in && state.passphrase == Some(true),
            // A new key only replaces one this device can open, or makes the first (encryption.rs).
            Self::NewRecoveryKey => signed_in && matches!(state.recovery, RecoveryStatus::On | RecoveryStatus::Off),
            Self::EnterRecoveryKey => signed_in && state.recovery == RecoveryStatus::NeedsKey,
        }
    }

    /// Whether it is a choice that is on now (the appearance in use).
    fn checked(self, state: &State) -> Option<bool> {
        match self {
            Self::Appearance(a) => Some(state.prefs.appearance == a),
            _ => None,
        }
    }
}

/// The catalog the runtime binds keys to and resolves menu rows from. `None` only if the runtime refused it (a
/// duplicate id; the test below rules that out).
pub fn catalog() -> Option<AppCatalog> {
    let defs = AppCommand::ALL
        .into_iter()
        .map(|c| {
            let def = CommandDef::new(c.id(), c.label(), Scope::app());
            match c.default_keys().and_then(|keys| Binding::parse(keys).ok()) {
                Some(binding) => def.with_default(binding),
                None => def,
            }
        })
        .collect();
    AppCatalog::new(defs, 1).map_err(|error| log::warn!("PingCraft's commands were refused: {error}")).ok()
}

/// Each command's state for this frame: whether it can run, and which appearance is on.
pub fn states(state: &State) -> CommandStates {
    let mut states = CommandStates::new();
    for c in AppCommand::ALL {
        let mut s = CommandState::new().enabled(c.enabled(state));
        if let Some(on) = c.checked(state) {
            s = s.checked(on);
        }
        states.set(c.id(), s);
    }
    states
}

/// The app's menus before the platform's layout: File, Edit (Settings, which each platform puts in its place) and
/// View. Quit and the platform's own items come from the layout.
pub fn menu_bar() -> MenuBar {
    let item = |c: AppCommand| Node::Item(Item::new(c.id(), c.label()));
    let appearance = Node::submenu(
        tl!("Appearance"),
        [Appearance::Light, Appearance::Dark, Appearance::System].into_iter().map(|a| item(AppCommand::Appearance(a))).collect(),
    )
    .role(ItemRole::Appearance);
    let mut bar = MenuBar::new(vec![
        Menu::new(tl!("File"), vec![item(AppCommand::SignOut)]),
        Menu::new(tl!("Edit"), vec![item(AppCommand::Settings)]),
        Menu::new(tl!("View"), vec![item(AppCommand::Inbox), item(AppCommand::Accounts), Node::Separator, item(AppCommand::Refresh), appearance]),
    ]);
    bar.tag_menu(tl!("File"), MenuRole::File);
    bar.tag_menu(tl!("Edit"), MenuRole::Edit);
    bar.tag_menu(tl!("View"), MenuRole::View);
    bar.tag_item(&CommandRef::new(AppCommand::Settings.id()), ItemRole::Settings);
    bar
}

/// The menus laid out for this build's platform, held for the in-window bar.
pub fn laid_out() -> Tracked<MenuBar> {
    Tracked::new(craft_menu::layout(&menu_bar(), Target::current(), &LayoutOptions::new("PingCraft")).bar)
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, catalog};

    /// Every command has its own id, and a fired reference finds its command again.
    #[test]
    fn each_command_has_its_own_id_and_is_found_by_it() {
        for c in AppCommand::ALL {
            assert_eq!(AppCommand::from_ref(&c.id().into()), Some(c), "{c:?}");
            assert_eq!(AppCommand::ALL.iter().filter(|o| o.id() == c.id()).count(), 1, "{c:?}");
        }
        assert!(catalog().is_some(), "the runtime takes the catalog");
    }
}

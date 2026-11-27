//! PingCraft's own commands (rule 14): the window's tabs, Settings and what its pages do, registered with craft-ui's
//! command board as an app catalog, so a shortcut, a menu row, a button on a page and (later) the automation interface
//! all run the same command. The app menu (craft-libs `menu`) lists them; craft-menu lays it out for the platform
//! (on Linux and Windows, Settings goes last in Edit as "Preferences…", as in the other Craft apps).
//!
//! Shortcuts (maintainer, 2026-11-26), in craft-keymap's words, where `Cmd` is ⌘ on macOS and Ctrl elsewhere and
//! `Ctrl` is the Control key on macOS (Ctrl elsewhere): Settings ⌘, (the Craft apps'); Inbox ⌘1 and Accounts ⌘2;
//! Search chats ⌘F (the Craft apps' find); Refresh F5 and ⌘R; the next and previous chat Ctrl+Tab / Ctrl+Shift+Tab,
//! ⌥↓ / ⌥↑ and ⌘PgDn / ⌘PgUp; the next and previous unread chat Ctrl+⌥Tab / Ctrl+⌥⇧Tab and ⌥⇧↓ / ⌥⇧↑; mark the
//! open chat read ⌘⇧A. Escape in an open chat goes back to the list when nothing else takes it (not a binding: the
//! window's own Escape, `lib.rs`).
//!
//! Settings' own actions that take a value (a network's sound, a device to verify) are [`SettingAction`]s: their ids
//! are registered too, and a fired reference carries the value in its parameters.

use craft_menu::{Item, ItemRole, LayoutOptions, Menu, MenuBar, MenuRole, Node, Target};
use craft_ui::contract::Tracked;
use craft_ui::runtime::commands::{AppCatalog, Binding, CommandDef, CommandRef, CommandState, CommandStates, Scope};
use serde_json::json;

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
    /// Opens the sheet that resets a lost recovery key.
    ResetRecovery,
    /// Opens the sheet that types in the account's recovery key.
    EnterRecoveryKey,
    /// Asks the user's other devices to verify this one.
    VerifyThisDevice,
    /// Opens the sheet that signs this device out.
    SignOut,
    /// Shows the inbox with its search field focused.
    Search,
    /// Opens the next chat in the inbox's order (with its filter and search), wrapping at the end.
    NextChat,
    /// Opens the previous chat, wrapping at the start.
    PreviousChat,
    /// Opens the next unread chat.
    NextUnread,
    /// Opens the previous unread chat.
    PreviousUnread,
    /// Marks the open chat read.
    MarkRead,
}

impl AppCommand {
    /// Every command, in the catalog's order.
    pub const ALL: [Self; 19] = [
        Self::Inbox,
        Self::Accounts,
        Self::Settings,
        Self::Refresh,
        Self::Appearance(Appearance::Light),
        Self::Appearance(Appearance::Dark),
        Self::Appearance(Appearance::System),
        Self::ChangePassphrase,
        Self::NewRecoveryKey,
        Self::ResetRecovery,
        Self::EnterRecoveryKey,
        Self::VerifyThisDevice,
        Self::SignOut,
        Self::Search,
        Self::NextChat,
        Self::PreviousChat,
        Self::NextUnread,
        Self::PreviousUnread,
        Self::MarkRead,
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
            Self::ResetRecovery => "security.reset_recovery",
            Self::EnterRecoveryKey => "security.enter_recovery_key",
            Self::VerifyThisDevice => "security.verify_this_device",
            Self::SignOut => "security.sign_out",
            Self::Search => "chats.search",
            Self::NextChat => "chats.next",
            Self::PreviousChat => "chats.previous",
            Self::NextUnread => "chats.next_unread",
            Self::PreviousUnread => "chats.previous_unread",
            Self::MarkRead => "chat.mark_read",
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
            Self::ResetRecovery => tl!("Reset: make a new recovery key…"),
            Self::EnterRecoveryKey => tl!("Enter recovery key…"),
            Self::VerifyThisDevice => tl!("Verify this device"),
            Self::SignOut => tl!("Sign out…"),
            Self::Search => tl!("Search chats"),
            Self::NextChat => tl!("Next chat"),
            Self::PreviousChat => tl!("Previous chat"),
            Self::NextUnread => tl!("Next unread chat"),
            Self::PreviousUnread => tl!("Previous unread chat"),
            Self::MarkRead => tl!("Mark as read"),
        }
    }

    /// Its default shortcuts, in craft-keymap's words; the first is the one menus show.
    fn default_keys(self) -> &'static [&'static str] {
        match self {
            Self::Settings => &["Cmd+,"],
            Self::Inbox => &["Cmd+1"],
            Self::Accounts => &["Cmd+2"],
            Self::Search => &["Cmd+F"],
            Self::Refresh => &["F5", "Cmd+R"],
            Self::NextChat => &["Ctrl+Tab", "Alt+Down", "Cmd+PageDown"],
            Self::PreviousChat => &["Ctrl+Shift+Tab", "Alt+Up", "Cmd+PageUp"],
            Self::NextUnread => &["Ctrl+Alt+Tab", "Alt+Shift+Down"],
            Self::PreviousUnread => &["Ctrl+Alt+Shift+Tab", "Alt+Shift+Up"],
            Self::MarkRead => &["Cmd+Shift+A"],
            _ => &[],
        }
    }

    /// Whether it can run now. A command that cannot fires nothing and is greyed in the menus.
    pub fn enabled(self, state: &State) -> bool {
        use pingcraft_core::encryption::RecoveryStatus;
        let signed_in = state.screen == Screen::Inbox;
        match self {
            Self::Inbox | Self::Accounts | Self::Settings | Self::Refresh | Self::VerifyThisDevice | Self::SignOut | Self::Search => signed_in,
            Self::NextChat | Self::PreviousChat | Self::NextUnread | Self::PreviousUnread => signed_in,
            Self::MarkRead => signed_in && state.open.is_some(),
            Self::Appearance(_) => true,
            Self::ChangePassphrase => signed_in && state.passphrase == Some(true),
            // A new key only replaces one this device can open, or makes the first (encryption.rs).
            Self::NewRecoveryKey => signed_in && matches!(state.recovery, RecoveryStatus::On | RecoveryStatus::Off),
            Self::EnterRecoveryKey | Self::ResetRecovery => signed_in && state.recovery == RecoveryStatus::NeedsKey,
        }
    }

    /// Whether it is a choice that is on now (the appearance in use).
    fn checked(self, state: &State) -> Option<bool> {
        match self {
            Self::Appearance(a) => Some(state.appearance() == a),
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
            c.default_keys()
                .iter()
                .filter_map(|keys| Binding::parse(keys).ok())
                .fold(CommandDef::new(c.id(), c.label(), Scope::app()), CommandDef::with_default)
        })
        .chain(SettingAction::IDS.iter().map(|(id, label)| CommandDef::new(*id, *label, Scope::app())))
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
        Menu::new(tl!("View"), vec![item(AppCommand::Refresh), appearance]),
        Menu::new(
            tl!("Go"),
            vec![
                item(AppCommand::Inbox),
                item(AppCommand::Accounts),
                item(AppCommand::Search),
                Node::Separator,
                item(AppCommand::NextChat),
                item(AppCommand::PreviousChat),
                item(AppCommand::NextUnread),
                item(AppCommand::PreviousUnread),
            ],
        ),
        Menu::new(tl!("Chat"), vec![item(AppCommand::MarkRead)]),
    ]);
    bar.tag_menu(tl!("File"), MenuRole::File);
    bar.tag_menu(tl!("Edit"), MenuRole::Edit);
    bar.tag_menu(tl!("View"), MenuRole::View);
    bar.tag_item(&CommandRef::new(AppCommand::Settings.id()), ItemRole::Settings);
    bar
}

/// One of Settings' actions that takes a value. Each runs as a [`AppCommand`] does; its id is registered, and a fired
/// reference carries the value in its parameters (for the automation interface to come).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingAction {
    /// Whether notifications show the message's text.
    Previews(bool),
    /// A network's notification sound, by network id.
    NetworkSound { network: String, on: bool },
    /// A network's accent colour, by network id, as `#rrggbb`.
    NetworkAccent { network: String, colour: String },
    /// The default skin tone, by its saved name.
    SkinTone(String),
    /// This device's name.
    RenameDevice(String),
    /// Asks another device, by its ID, to verify with this one.
    VerifyDevice(String),
    /// Opens the sheet that signs another device, by its ID, out.
    SignOutDevice(String),
    /// Imports the theme file at this path into the suite library.
    ImportTheme(String),
}

impl SettingAction {
    /// The ids, with their names in the command palette.
    pub const IDS: [(&'static str, &'static str); 8] = [
        ("notifications.previews", "Show message text in notifications"),
        ("notifications.network_sound", "Network notification sound"),
        ("notifications.network_accent", "Network accent colour"),
        ("emoji.skin_tone", "Default skin tone"),
        ("security.rename_device", "Rename this device"),
        ("security.verify_device", "Verify a device"),
        ("security.sign_out_device", "Sign a device out…"),
        ("appearance.import_theme", "Import a theme"),
    ];

    /// The reference it fires as: its id and its value.
    pub fn to_ref(&self) -> CommandRef {
        let (id, params) = match self {
            Self::Previews(on) => ("notifications.previews", json!({ "on": on })),
            Self::NetworkSound { network, on } => ("notifications.network_sound", json!({ "network": network, "on": on })),
            Self::NetworkAccent { network, colour } => ("notifications.network_accent", json!({ "network": network, "colour": colour })),
            Self::SkinTone(tone) => ("emoji.skin_tone", json!({ "tone": tone })),
            Self::RenameDevice(name) => ("security.rename_device", json!({ "name": name })),
            Self::VerifyDevice(device) => ("security.verify_device", json!({ "device": device })),
            Self::SignOutDevice(device) => ("security.sign_out_device", json!({ "device": device })),
            Self::ImportTheme(path) => ("appearance.import_theme", json!({ "path": path })),
        };
        CommandRef::with_params(id, params)
    }

    /// The action a fired reference names, with its value; `None` for another command or a value missing.
    pub fn from_ref(command: &CommandRef) -> Option<Self> {
        let p = &command.params;
        let text = |key: &str| p.get(key).and_then(serde_json::Value::as_str).map(str::to_owned);
        let flag = |key: &str| p.get(key).and_then(serde_json::Value::as_bool);
        Some(match command.id.as_str() {
            "notifications.previews" => Self::Previews(flag("on")?),
            "notifications.network_sound" => Self::NetworkSound { network: text("network")?, on: flag("on")? },
            "notifications.network_accent" => Self::NetworkAccent { network: text("network")?, colour: text("colour")? },
            "emoji.skin_tone" => Self::SkinTone(text("tone")?),
            "security.rename_device" => Self::RenameDevice(text("name")?),
            "security.verify_device" => Self::VerifyDevice(text("device")?),
            "security.sign_out_device" => Self::SignOutDevice(text("device")?),
            "appearance.import_theme" => Self::ImportTheme(text("path")?),
            _ => return None,
        })
    }
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
        for c in AppCommand::ALL {
            for keys in c.default_keys() {
                assert!(super::Binding::parse(keys).is_ok(), "{c:?}: {keys}");
            }
        }
    }

    /// A Settings action fired with its value comes back as the same action.
    #[test]
    fn a_setting_action_carries_its_value() {
        use super::SettingAction;
        for a in [
            SettingAction::Previews(false),
            SettingAction::NetworkSound { network: "signal".into(), on: true },
            SettingAction::NetworkAccent { network: "telegram".into(), colour: "#123456".into() },
            SettingAction::SkinTone("dark".into()),
            SettingAction::RenameDevice("Laptop".into()),
            SettingAction::VerifyDevice("PHONE".into()),
            SettingAction::SignOutDevice("PHONE".into()),
            SettingAction::ImportTheme("/tmp/theme.yaml".into()),
        ] {
            assert_eq!(SettingAction::from_ref(&a.to_ref()), Some(a.clone()), "{a:?}");
        }
        assert_eq!(SettingAction::from_ref(&"security.verify_device".into()), None, "a value is needed");
    }
}

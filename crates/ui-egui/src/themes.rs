//! Colour themes (PLAN.md, M6 ticket 7): craft-libs' built-ins, the suite library's themes (craft-theme-store), and
//! PingCraft's own copies of the palette themes proposed to craft-libs (storytold/craft-libs#3701, #3704 to #3709),
//! carried until craft-libs ships them.
//!
//! The copies are the same `craft-design-theme` files as the pull requests, read with craft-theme's own parser.
//! Their `craft.` ids are reserved for craft-libs' built-ins, so PingCraft gives each its own id under
//! [`PREFIX`] (`craft.catppuccin` is `pingcraft.catppuccin`); once craft-libs ships one, [`builtin_for`] says which
//! built-in a saved choice of it becomes.

use std::sync::LazyLock;

use craft_theme::{AppChoice, AppDefaults, AppSizes, ComponentSet, ModeChoice, Selection, Size, SuiteChoice, ThemeDef, ThemeId, ThemePair};

/// The id prefix of PingCraft's own themes.
pub const PREFIX: &str = "pingcraft.";

/// The files, by the name of their palette.
const FILES: [(&str, &[u8]); 7] = [
    ("catppuccin", include_bytes!("../themes/catppuccin.json")),
    ("dracula", include_bytes!("../themes/dracula.json")),
    ("gruvbox", include_bytes!("../themes/gruvbox.json")),
    ("nord", include_bytes!("../themes/nord.json")),
    ("one-dark", include_bytes!("../themes/one-dark.json")),
    ("solarized-light", include_bytes!("../themes/solarized-light.json")),
    ("tokyo-night", include_bytes!("../themes/tokyo-night.json")),
];

/// PingCraft's own themes, read once. A file craft-theme refuses is left out (a test makes sure none is).
pub fn own() -> &'static [ThemeDef] {
    static OWN: LazyLock<Vec<ThemeDef>> = LazyLock::new(|| {
        FILES
            .iter()
            .filter_map(|(name, bytes)| {
                let mut theme = match craft_theme::format::parse(bytes) {
                    Ok(file) => file.theme,
                    Err(errors) => {
                        log::warn!("PingCraft's {name} theme was refused: {errors:?}");
                        return None;
                    }
                };
                theme.id = ThemeId::parse(&format!("{PREFIX}{name}")).ok()?;
                Some(theme)
            })
            .collect()
    });
    &OWN
}

/// One of PingCraft's own themes, by id.
pub fn own_theme(id: &ThemeId) -> Option<&'static ThemeDef> {
    own().iter().find(|t| t.id == *id)
}

/// The craft-libs built-in a theme of PingCraft's own becomes once craft-libs ships it: `craft.<name>`.
pub fn builtin_for(id: &ThemeId) -> Option<ThemeId> {
    let name = id.as_str().strip_prefix(PREFIX)?;
    ThemeId::parse(&format!("craft.{name}")).ok().filter(|b| craft_theme::builtins().iter().any(|t| t.id == *b))
}

/// What PingCraft shows when nothing is chosen: craft-libs' default theme, following the system's mode.
pub fn defaults() -> AppDefaults {
    AppDefaults {
        theme: ThemePair::same(ThemeId::suite_default()),
        mode: ModeChoice::System,
        size: Size::default(),
        sizes: AppSizes::default(),
        set: ComponentSet::default(),
    }
}

/// No choice of PingCraft's own: everything follows the suite.
pub fn follow_suite() -> AppChoice {
    AppChoice::new(None, None, None)
}

/// The selection shown: PingCraft's own choice over the suite's, the defaults for the rest.
pub fn selection(suite: &SuiteChoice, app: &AppChoice) -> Selection {
    app.apply(&suite.with_defaults(&defaults()))
}

#[cfg(test)]
mod tests {
    use super::{PREFIX, builtin_for, own};

    /// Every file is read, each under its own PingCraft id, never a reserved `craft.` one.
    #[test]
    fn every_own_theme_is_read_under_its_own_id() {
        assert_eq!(own().len(), 7);
        for theme in own() {
            assert!(theme.id.as_str().starts_with(PREFIX), "{}", theme.id);
            assert!(!theme.id.is_builtin());
            assert!(theme.validate().is_ok(), "{}", theme.id);
            // Until craft-libs ships it, there is no built-in to move to.
            assert!(builtin_for(&theme.id).is_none() || craft_theme::builtins().iter().any(|b| Some(&b.id) == builtin_for(&theme.id).as_ref()));
        }
    }
}

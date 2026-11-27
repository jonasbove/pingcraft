//! Theme import (PLAN.md, M6 ticket 7): a theme file from elsewhere, made into a `craft-design-theme` file that
//! craft-theme-store keeps in the suite library, where every Craft app finds it.
//!
//! - **Craft theme JSON** (`craft-design-theme`) is taken as it is; the library checks it.
//! - **base16 and base24 schemes** (YAML): the sixteen base colours map onto craft-theme's seeds and pins: base00 the
//!   window, base01 fields, base02 lines and hover, base03 faint text and control edges, base04 muted text, base05 text,
//!   base08 bad, base0A warn, base0B good, base0C info, base0D the accent. base24's extra colours are not needed.
//! - **VS Code colour themes** (JSON with comments): a best-effort mapping of the workbench colours onto the same
//!   tokens; the colours it needs and cannot find are listed, and craft-theme derives them instead.
//!
//! Nothing here touches the disk: the app sends the result to the library (`Op::Import`).

use std::collections::BTreeMap;

use craft_theme::Rgba;
use serde_json::{Value, json};

/// The largest theme file read, in bytes. Theme files are a few kilobytes.
pub const MAX_FILE: usize = 512 * 1024;

/// A theme made ready for the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    /// The `craft-design-theme` file.
    pub file: String,
    /// The theme's name.
    pub name: String,
    /// What the source did not say, which craft-theme derives instead (a VS Code theme's missing colours).
    pub unmapped: Vec<&'static str>,
}

/// Why a file could not be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    TooLarge,
    /// Not a format PingCraft reads.
    Unknown,
    /// The format is known but a colour it needs is missing or not a colour.
    Missing(&'static str),
    /// craft-theme refused the result (a theme file that is itself wrong).
    Refused(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("the file is too large to be a theme"),
            Self::Unknown => f.write_str("it is not a Craft theme, a base16 or base24 scheme, or a VS Code colour theme"),
            Self::Missing(what) => write!(f, "it has no usable {what}"),
            Self::Refused(why) => write!(f, "the theme is not valid: {why}"),
        }
    }
}

/// Makes the file named `name` with `bytes` into a theme file for the library.
pub fn convert(name: &str, bytes: &[u8]) -> Result<Converted, ImportError> {
    if bytes.len() > MAX_FILE {
        return Err(ImportError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ImportError::Unknown)?;
    let lower = name.to_ascii_lowercase();
    let converted = if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        base16(&flat_yaml(text), name)?
    } else {
        let value = craft_jsonc::parse_bounded(text, false, MAX_FILE).map_err(|_| ImportError::Unknown)?.to_value();
        if value.get("schema").and_then(Value::as_str) == Some("craft-design-theme") {
            let theme = craft_theme::format::parse(bytes).map_err(|e| ImportError::Refused(format!("{e:?}")))?.theme;
            return Ok(Converted { file: text.to_owned(), name: theme.name, unmapped: Vec::new() });
        }
        if value.get("colors").is_some() || value.get("tokenColors").is_some() {
            vscode(&value, name)?
        } else {
            return Err(ImportError::Unknown);
        }
    };
    // Checked as the library will check it.
    craft_theme::format::parse(converted.file.as_bytes()).map_err(|e| ImportError::Refused(format!("{e:?}")))?;
    Ok(converted)
}

/// The scalars of a flat YAML document, one `key: value` a line, quoted or not; one level of nesting (a newer
/// scheme's `palette:` block) is read into the same map. Comments and blank lines are skipped. This is all a base16 or
/// base24 scheme file holds, so no YAML library is needed.
fn flat_yaml(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines().take(1000) {
        let line = line.split(" #").next().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else { continue };
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        if value.is_empty() {
            continue;
        }
        map.insert(key.trim().trim_matches('"').to_owned(), value.to_owned());
    }
    map
}

/// A colour as `#rrggbb`, from `rrggbb`, `#rrggbb`, `#rgb` or `#rrggbbaa` (the alpha dropped: surfaces are opaque).
fn colour(text: &str) -> Option<Rgba> {
    let hex = text.trim().trim_start_matches('#');
    let full = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_owned(),
        8 => hex.get(..6)?.to_owned(),
        _ => return None,
    };
    let byte = |at: usize| full.get(at..at + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    Some(Rgba([byte(0)?, byte(2)?, byte(4)?, 0xff]))
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0[0], c.0[1], c.0[2])
}

/// An id of the theme's own, from its name: `imported.<letters and digits>`.
fn id_for(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug: String = slug.chars().take(40).collect();
    format!("imported.{}", if slug.is_empty() { "theme" } else { &slug })
}

/// Whether a colour is dark (its relative luminance below the middle).
fn is_dark(c: Rgba) -> bool {
    craft_theme::relative_luminance(c) < 0.18
}

/// The colours a theme file is written from. What a source does not give is left out, and craft-theme derives it
/// (the seeds from craft-libs' default theme).
struct Palette {
    name: String,
    background: Rgba,
    text: Rgba,
    panel: Option<Rgba>,
    sunken: Option<Rgba>,
    line: Option<Rgba>,
    edge: Option<Rgba>,
    hover: Option<Rgba>,
    muted: Option<Rgba>,
    faint: Option<Rgba>,
    accent: Option<Rgba>,
    bad: Option<Rgba>,
    warn: Option<Rgba>,
    good: Option<Rgba>,
    info: Option<Rgba>,
}

impl Palette {
    /// The `craft-design-theme` file, in the one mode the palette has.
    fn file(&self) -> String {
        let dark = is_dark(self.background);
        let mode = if dark { "dark" } else { "light" };
        let default = craft_theme::builtins()
            .iter()
            .find(|t| t.id == craft_theme::ThemeId::suite_default())
            .map(|t| t.seeds_for(if dark { craft_theme::Mode::Dark } else { craft_theme::Mode::Light }));
        let seed = |own: Option<Rgba>, pick: fn(&craft_theme::Seeds) -> Rgba| own.or_else(|| default.as_ref().map(pick)).map(hex);
        let mut pins = serde_json::Map::new();
        let mut pin = |token: &str, c: Option<Rgba>| {
            if let Some(c) = c {
                pins.insert(token.to_owned(), json!(hex(c)));
            }
        };
        pin("surface.background", Some(self.background));
        pin("surface.canvas", Some(self.background));
        pin("surface.panel", self.panel);
        pin("surface.sunken", self.sunken);
        pin("line.divider", self.line);
        pin("line.border", self.line);
        pin("line.control_edge", self.edge);
        pin("state.hover", self.hover);
        pin("text.normal", Some(self.text));
        pin("text.muted", self.muted);
        pin("text.faint", self.faint);
        pin("accent.solid", self.accent);
        if let Some(accent) = self.accent {
            pins.insert("accent.soft".to_owned(), json!(format!("{}2e", hex(accent))));
            pins.insert("state.selected".to_owned(), json!("{accent.soft}"));
        }
        let theme = json!({
            "schema": "craft-design-theme",
            "schema_version": 1,
            "id": id_for(&self.name),
            "name": self.name,
            "version": 1,
            "variants": mode,
            "contrast_intent": "standard",
            "seeds": {
                "accent": seed(self.accent, |s| s.accent),
                "neutral": hex(self.background),
                "status": {
                    "bad": seed(self.bad, |s| s.status.bad),
                    "good": seed(self.good, |s| s.status.good),
                    "info": seed(self.info, |s| s.status.info),
                    "warn": seed(self.warn, |s| s.status.warn),
                },
            },
            "shape": null,
            (mode): pins,
        });
        serde_json::to_string_pretty(&theme).unwrap_or_default()
    }
}

/// A base16 or base24 scheme.
fn base16(map: &BTreeMap<String, String>, file_name: &str) -> Result<Converted, ImportError> {
    let base = |n: &'static str| map.get(n).and_then(|v| colour(v)).ok_or(ImportError::Missing(n));
    let name = map
        .get("scheme")
        .or_else(|| map.get("name"))
        .cloned()
        .unwrap_or_else(|| file_name.trim_end_matches(".yaml").trim_end_matches(".yml").to_owned());
    let palette = Palette {
        name,
        background: base("base00")?,
        text: base("base05")?,
        panel: Some(base("base00")?),
        sunken: Some(base("base01")?),
        line: Some(base("base02")?),
        edge: Some(base("base03")?),
        hover: Some(base("base02")?),
        muted: Some(base("base04")?),
        faint: Some(base("base03")?),
        accent: Some(base("base0D")?),
        bad: Some(base("base08")?),
        warn: Some(base("base0A")?),
        good: Some(base("base0B")?),
        info: Some(base("base0C")?),
    };
    Ok(Converted { file: palette.file(), name: palette.name, unmapped: Vec::new() })
}

/// A VS Code colour theme: the workbench colours that best match each token, the first one present winning.
fn vscode(value: &Value, file_name: &str) -> Result<Converted, ImportError> {
    let colors = value.get("colors").and_then(Value::as_object);
    let find = |keys: &[&str]| keys.iter().find_map(|k| colors.and_then(|c| c.get(*k)).and_then(Value::as_str).and_then(colour));
    let mut unmapped = Vec::new();
    let background = find(&["editor.background"]).ok_or(ImportError::Missing("editor.background"))?;
    let text = find(&["editor.foreground", "foreground"]).ok_or(ImportError::Missing("editor.foreground"))?;
    let mut want = |what: &'static str, keys: &[&str]| {
        let found = find(keys);
        if found.is_none() {
            unmapped.push(what);
        }
        found
    };
    let panel = want("panels", &["sideBar.background", "panel.background", "activityBar.background"]);
    let sunken = want("fields", &["input.background", "editorWidget.background"]);
    let line = want("dividers", &["panel.border", "sideBar.border", "editorGroup.border", "contrastBorder"]);
    let edge = want("control edges", &["input.border", "focusBorder", "contrastBorder"]);
    let hover = want("hover", &["list.hoverBackground", "toolbar.hoverBackground"]);
    let muted = want("muted text", &["descriptionForeground", "editorLineNumber.activeForeground"]);
    let faint = want("faint text", &["editorLineNumber.foreground", "disabledForeground"]);
    let accent = want("accent", &["button.background", "focusBorder", "activityBarBadge.background", "textLink.foreground"]);
    let bad = want("error", &["errorForeground", "editorError.foreground", "terminal.ansiRed"]);
    let warn = want("warning", &["editorWarning.foreground", "terminal.ansiYellow"]);
    let good = want("success", &["terminal.ansiGreen", "gitDecoration.addedResourceForeground"]);
    let info = want("information", &["editorInfo.foreground", "terminal.ansiBlue"]);
    let name = value.get("name").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| file_name.trim_end_matches(".json").to_owned());
    let palette = Palette { name, background, text, panel, sunken, line, edge, hover, muted, faint, accent, bad, warn, good, info };
    Ok(Converted { file: palette.file(), name: palette.name, unmapped })
}

#[cfg(test)]
mod tests {
    use super::{ImportError, colour, convert, id_for};

    const BASE16: &str = "# A scheme of our own, for the tests.\nscheme: \"Harbour Night\"\nauthor: \"PingCraft tests\"\nbase00: \"1b1d24\"\nbase01: \"23262f\"\nbase02: \"2e323d\"\nbase03: \"4b5263\"\nbase04: \"8b93a6\"\nbase05: \"d4d8e2\"\nbase06: \"e3e6ee\"\nbase07: \"f2f4f8\"\nbase08: \"e06c75\"\nbase09: \"d19a66\"\nbase0A: \"e5c07b\"\nbase0B: \"98c379\"\nbase0C: \"56b6c2\"\nbase0D: \"61afef\"\nbase0E: \"c678dd\"\nbase0F: \"be5046\"\n";

    const VSCODE: &str = r##"{
        // A theme of our own, for the tests; comments as VS Code allows.
        "name": "Paper Morning",
        "type": "light",
        "colors": {
            "editor.background": "#fbf8f1",
            "editor.foreground": "#2b2a27",
            "sideBar.background": "#f3efe4",
            "input.background": "#ffffff",
            "button.background": "#2f6fbf",
            "errorForeground": "#b3261e",
        },
        "tokenColors": []
    }"##;

    #[test]
    fn a_base16_scheme_becomes_a_dark_theme_the_library_takes() {
        let c = convert("harbour.yaml", BASE16.as_bytes()).unwrap();
        assert_eq!(c.name, "Harbour Night");
        let theme = craft_theme::format::parse(c.file.as_bytes()).unwrap().theme;
        assert_eq!(theme.id.as_str(), "imported.harbour-night");
        assert!(!theme.id.is_builtin(), "never a reserved id");
        assert_eq!(theme.variants, craft_theme::Variants::DarkOnly, "a dark background makes a dark theme");
    }

    #[test]
    fn a_vscode_theme_maps_what_it_has_and_says_what_it_lacks() {
        let c = convert("paper.json", VSCODE.as_bytes()).unwrap();
        let theme = craft_theme::format::parse(c.file.as_bytes()).unwrap().theme;
        assert_eq!(theme.variants, craft_theme::Variants::LightOnly);
        assert_eq!(theme.name, "Paper Morning");
        assert!(c.unmapped.contains(&"warning") && !c.unmapped.contains(&"accent"), "{:?}", c.unmapped);
    }

    #[test]
    fn a_craft_theme_is_taken_as_it_is_and_nonsense_is_refused() {
        let craft = include_str!("../themes/nord.json").replace("craft.nord", "mine.nord");
        assert_eq!(convert("nord.json", craft.as_bytes()).map(|c| c.file), Ok(craft));
        assert_eq!(convert("notes.txt", b"hello"), Err(ImportError::Unknown));
        assert_eq!(convert("x.json", b"{\"colors\": {}}"), Err(ImportError::Missing("editor.background")));
        assert!(matches!(convert("x.yaml", b"scheme: nothing\n"), Err(ImportError::Missing(_))));
        assert_eq!(convert("big.json", &vec![b' '; super::MAX_FILE + 1]), Err(ImportError::TooLarge));
    }

    #[test]
    fn colours_and_ids_are_read_carefully() {
        assert_eq!(colour("#abc"), colour("aabbcc"));
        assert_eq!(colour("#11223344").map(|c| c.0), Some([0x11, 0x22, 0x33, 0xff]), "alpha dropped");
        assert_eq!(colour("#12345"), None);
        assert_eq!(colour("#zzzzzz"), None);
        assert_eq!(id_for("Tokyo Night (Storm)!"), "imported.tokyo-night-storm");
        assert_eq!(id_for("✨"), "imported.theme");
    }
}

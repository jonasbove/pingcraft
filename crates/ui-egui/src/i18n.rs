//! Translations. Every piece of interface text is English source text looked up with [`tl!`], counts go through
//! [`trn`] and `{name}` placeholders through [`fmt`], in the call shape of craft-libs' `craft-i18n`, so it can
//! take over here without touching the screens.
//!
//! For now this is a stand-in with no catalogs: the English is shown as is. `craft-i18n` cannot be linked into the
//! same program as `matrix-sdk` 0.19.1 yet: its ICU crates turn on `zerovec`'s `serde` feature, and with it
//! `ruma-common` 0.20.2 fails to compile (E0275, type inference overflows on `Box::deserialize`). PLAN.md tracks
//! it. What is missing until then: catalogs, the system language, and bidi isolation of inserted names.

/// Looks English interface text up in the current language (English only, for now).
pub fn t(source: &str) -> &str {
    source
}

/// Shows the interface in the system's language when it is one PingCraft offers. None is offered yet.
pub fn use_system_language() {}

/// Fills `{name}` placeholders in one pass. A placeholder without a value is left as it is.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').map(|close| (&after[..close], close)) {
            Some((name, close)) if args.iter().any(|(n, _)| *n == name) => {
                out.push_str(args.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).unwrap_or_default());
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A count in words: `one` for 1, `other` otherwise, with `{n}` filled in (English plural rules for now).
pub fn trn(n: u64, one: &str, other: &str) -> String {
    let template = if n == 1 { t(one) } else { t(other) };
    fmt(template, &[("n", &n.to_string())])
}

/// Looks English interface text up in the current language.
#[macro_export]
macro_rules! tl {
    ($s:expr) => {
        $crate::i18n::t($s)
    };
}

#[cfg(test)]
mod tests {
    use super::{fmt, trn};

    #[test]
    fn placeholders_fill_once_and_unknown_ones_stay() {
        assert_eq!(fmt("{chat}, on {network}", &[("chat", "Book {network}"), ("network", "Signal")]), "Book {network}, on Signal");
        assert_eq!(fmt("{missing} and { and }", &[]), "{missing} and { and }");
        assert_eq!(trn(1, "{n} chat", "{n} chats"), "1 chat");
        assert_eq!(trn(2, "{n} chat", "{n} chats"), "2 chats");
    }
}

//! The account's emoji settings that the specification has no event for: for now the skin tone chosen in the
//! picker, in PingCraft's own global account data ([`EMOJI`], `{"skin_tone": "medium"}`), so it follows the user
//! between devices. Not in `m.recent_emoji`: its content has only `recent_emoji` in the specification, and apps
//! that read it into a typed struct would drop an extra key the next time they write.
//!
//! The tone is a name the interface knows (`SkinTone::id`); this side only checks it is a short text. "The account
//! has none" is only ever said after asking the server itself, never because the sync has not brought one yet, so
//! a device that starts before its first sync does not take a tone it kept locally for the account's.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::account_data::synced;
use crate::connection::Connection;

/// PingCraft's global account data for emoji.
pub(crate) const EMOJI: &str = crate::namespaced!("emoji");
/// The longest tone name taken from the server; anything longer is not one of PingCraft's.
const MAX_TONE: usize = 32;
/// How long to wait before trying again after a failed request (offline, say).
const RETRY: Duration = Duration::from_secs(30);
/// How long a save or a fetch may take in all.
pub const TIMEOUT: Duration = Duration::from_secs(12);

/// The skin tone in the content, when it is a short text.
pub(crate) fn skin_tone(content: &Value) -> Option<String> {
    content.get("skin_tone").and_then(Value::as_str).filter(|t| !t.is_empty() && t.len() <= MAX_TONE).map(str::to_owned)
}

/// What to do next about the skin tone.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    /// Send this tone, chosen on this device.
    Save(String),
    /// Ask the server which tone the account keeps (once signed in, until it has answered).
    Fetch,
    /// Look at the synced copy for another device's change.
    Watch,
    /// Nothing now (waiting to try again).
    Wait,
}

/// The skin tone between this device and the account: what the interface was told, a choice not sent yet, and
/// when to try again after a failure.
#[derive(Debug, Default)]
pub struct ToneSync {
    /// The server has said which tone the account keeps (or that it keeps none).
    known: bool,
    /// The tone the interface was last told or chose: a synced copy that differs is another device's change.
    shown: Option<String>,
    /// Chosen on this device and not sent yet.
    pending: Option<String>,
    not_before: Option<Instant>,
}

impl ToneSync {
    /// What to do now.
    pub fn next(&self, now: Instant) -> Next {
        if self.not_before.is_some_and(|at| now < at) {
            return Next::Wait;
        }
        match (&self.pending, self.known) {
            (Some(tone), _) => Next::Save(tone.clone()),
            (None, false) => Next::Fetch,
            (None, true) => Next::Watch,
        }
    }

    /// The user chose a tone on this device: sent at the next chance, and not told back to the interface.
    pub fn chosen(&mut self, tone: String) {
        self.shown = Some(tone.clone());
        self.pending = Some(tone);
        self.not_before = None;
    }

    /// The tone chosen here was sent.
    pub fn saved(&mut self) {
        self.pending = None;
        self.not_before = None;
    }

    /// The server said which tone the account keeps: what to tell the interface, unless a choice made here
    /// meanwhile is still to be sent (it wins).
    pub fn fetched(&mut self, tone: Option<String>) -> Option<Option<String>> {
        self.known = true;
        self.not_before = None;
        if self.pending.is_some() {
            return None;
        }
        self.shown.clone_from(&tone);
        Some(tone)
    }

    /// The synced copy is `tone`: the tone to tell the interface when another device changed it.
    pub fn synced(&mut self, tone: Option<String>) -> Option<String> {
        // A synced copy without a tone says nothing: the sync may not have brought it.
        let tone = tone?;
        if !self.known || self.pending.is_some() || self.shown.as_ref() == Some(&tone) {
            return None;
        }
        self.shown = Some(tone.clone());
        Some(tone)
    }

    /// A request failed: try again later.
    pub fn failed(&mut self, now: Instant) {
        self.not_before = Some(now + RETRY);
    }

    /// A tone chosen here and still to be sent (sent at once when PingCraft closes).
    pub fn pending(&self) -> Option<String> {
        self.pending.clone()
    }
}

fn problem(e: impl std::fmt::Display) -> String {
    format!("the skin tone could not be saved: {e}")
}

impl Connection {
    /// The skin tone in the synced copy (no request); `None` also when the sync has not brought one.
    pub async fn synced_skin_tone(&self) -> Option<String> {
        synced(&self.client().account(), EMOJI).await.as_ref().and_then(skin_tone)
    }

    /// The skin tone the account keeps, asked of the server now; `Ok(None)` when it keeps none.
    pub async fn fetch_skin_tone(&self) -> Result<Option<String>, String> {
        match tokio::time::timeout(TIMEOUT, self.fetch_global(EMOJI)).await {
            Ok(Ok(content)) => Ok(content.as_ref().and_then(skin_tone)),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("the server took too long".to_owned()),
        }
    }

    /// Keeps this skin tone with the account, on top of what the server has (other keys are kept).
    pub async fn save_skin_tone(&self, tone: &str) -> Result<(), String> {
        let change = self.merge_global(EMOJI, |content| {
            content.insert("skin_tone".to_owned(), json!(tone));
        });
        match tokio::time::timeout(TIMEOUT, change).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(problem(e)),
            Err(_) => Err(problem("the server took too long")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::{EMOJI, Next, ToneSync, skin_tone};

    /// The name other devices and later versions find the tone under: it must not change by accident.
    #[test]
    fn the_event_is_in_pingcrafts_namespace() {
        assert_eq!(EMOJI, "io.github.jonasbove.pingcraft.emoji");
        assert!(EMOJI.starts_with(&format!("{}.", crate::NAMESPACE)));
    }

    #[test]
    fn only_a_short_text_is_a_tone() {
        assert_eq!(skin_tone(&json!({ "skin_tone": "medium", "x.other": 1 })).as_deref(), Some("medium"));
        assert_eq!(skin_tone(&json!({ "skin_tone": "" })), None);
        assert_eq!(skin_tone(&json!({ "skin_tone": 3 })), None);
        assert_eq!(skin_tone(&json!({ "skin_tone": "x".repeat(33) })), None);
        assert_eq!(skin_tone(&json!({})), None);
        assert_eq!(skin_tone(&json!([])), None);
    }

    /// Signed in: ask the server first; after a failure, wait; once it answered, watch the sync.
    #[test]
    fn the_server_is_asked_first_and_again_after_a_failure() {
        let now = Instant::now();
        let mut tone = ToneSync::default();
        assert_eq!(tone.next(now), Next::Fetch);
        tone.failed(now);
        assert_eq!(tone.next(now + Duration::from_secs(5)), Next::Wait);
        assert_eq!(tone.next(now + Duration::from_secs(31)), Next::Fetch);
        assert_eq!(tone.fetched(None), Some(None), "the interface hears the account has none");
        assert_eq!(tone.next(now), Next::Watch);
    }

    /// A synced copy with nothing in it says nothing; another device's change is told once; this device's own
    /// choice is not told back.
    #[test]
    fn another_devices_change_is_told_once_and_this_ones_not_at_all() {
        let mut tone = ToneSync::default();
        assert_eq!(tone.synced(Some("dark".into())), None, "not before the server has answered");
        assert_eq!(tone.fetched(Some("dark".into())), Some(Some("dark".into())));
        assert_eq!(tone.synced(Some("dark".into())), None, "the same tone");
        assert_eq!(tone.synced(None), None);
        assert_eq!(tone.synced(Some("light".into())), Some("light".into()), "another device chose it");
        tone.chosen("medium".into());
        assert_eq!(tone.next(Instant::now()), Next::Save("medium".into()));
        assert_eq!(tone.synced(Some("light".into())), None, "the choice made here is still to be sent");
        tone.saved();
        assert_eq!(tone.pending(), None);
        assert_eq!(tone.synced(Some("medium".into())), None, "its own choice coming back");
    }

    /// A choice made before the server answered wins over what the server says, and is sent.
    #[test]
    fn a_choice_made_before_the_server_answered_wins() {
        let mut tone = ToneSync::default();
        tone.chosen("dark".into());
        assert_eq!(tone.fetched(None), None, "the interface keeps its choice");
        assert_eq!(tone.next(Instant::now()), Next::Save("dark".into()));
        let now = Instant::now();
        tone.failed(now);
        assert_eq!(tone.next(now), Next::Wait);
        assert_eq!(tone.pending().as_deref(), Some("dark"), "still sent when PingCraft closes");
    }
}

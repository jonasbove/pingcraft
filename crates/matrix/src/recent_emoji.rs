//! The account's recently used emoji, kept on the server so they follow the user between devices and survive a
//! restart: the Matrix specification's `m.recent_emoji` global account data (added in v1.18, "Recently used emoji").
//! Element Web writes the same event; an account that only has Element's older `io.element.recent_emoji` (a list of
//! `[emoji, count]` pairs) starts from that, and PingCraft writes only `m.recent_emoji`, as Element now does.
//!
//! Each use moves the emoji to the front of the list and adds one to its count. Uses are gathered for a few seconds
//! ([`Pending`]) and then sent together, on top of what the server has at that moment (fetched fresh, not from the
//! cache), so a change another device made in the meantime is kept, not overwritten. Entries PingCraft does not
//! understand and keys it does not know are kept as they are (the specification asks for that); malformed entries
//! (no text, a negative or missing count) are dropped, and the list is cut to [`MAX`] from its end.

use std::time::{Duration, Instant};

use matrix_sdk::ruma::events::GlobalAccountDataEventType;
use matrix_sdk::ruma::serde::Raw;
use serde_json::{Map, Value, json};

use crate::connection::Connection;

/// The event type in the specification.
pub(crate) const RECENT_EMOJI: &str = "m.recent_emoji";
/// Element's type from before the specification had one; read, never written.
pub(crate) const LEGACY_RECENT_EMOJI: &str = "io.element.recent_emoji";
/// How long the list may be: the specification's recommended maximum.
pub const MAX: usize = 100;
/// How long after the last use the uses are sent (so a burst of reactions is one write, not one each).
pub const QUIET: Duration = Duration::from_secs(3);
/// How long to wait before trying again when sending them failed (offline, say).
const RETRY: Duration = Duration::from_secs(30);

/// A well-formed entry: an object with a text `emoji` and a `total` of zero or more.
fn well_formed(entry: &Value) -> bool {
    entry.get("emoji").is_some_and(Value::is_string) && entry.get("total").is_some_and(Value::is_u64)
}

/// The content to build on: `m.recent_emoji`'s, or, when the account has none, one made from Element's older event.
pub(crate) fn base(current: Option<Value>, legacy: Option<Value>) -> Value {
    let mut content = match current {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    if content.contains_key("recent_emoji") {
        return Value::Object(content);
    }
    let pairs = legacy.as_ref().and_then(|l| l.get("recent_emoji")).and_then(Value::as_array).cloned().unwrap_or_default();
    let list: Vec<Value> = pairs
        .iter()
        .filter_map(|pair| {
            let [emoji, total] = pair.as_array()?.as_slice() else { return None };
            Some(json!({ "emoji": emoji.as_str()?, "total": total.as_u64()? }))
        })
        .collect();
    content.insert("recent_emoji".to_owned(), Value::Array(list));
    Value::Object(content)
}

/// `content` with each of `uses` (oldest first) moved to the front and counted, malformed entries dropped and the
/// list cut to [`MAX`]. Other keys and entries are kept as they are.
pub(crate) fn with_uses(content: Value, uses: &[String]) -> Value {
    let mut content = match content {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let mut list: Vec<Value> = match content.remove("recent_emoji") {
        Some(Value::Array(list)) => list.into_iter().filter(well_formed).collect(),
        _ => Vec::new(),
    };
    for emoji in uses {
        let entry = match list.iter().position(|e| e.get("emoji").and_then(Value::as_str) == Some(emoji)) {
            Some(at) => {
                let mut entry = list.remove(at);
                let total = entry.get("total").and_then(Value::as_u64).unwrap_or(0).saturating_add(1);
                if let Some(map) = entry.as_object_mut() {
                    map.insert("total".to_owned(), json!(total));
                }
                entry
            }
            None => json!({ "emoji": emoji, "total": 1 }),
        };
        list.insert(0, entry);
    }
    list.truncate(MAX);
    content.insert("recent_emoji".to_owned(), Value::Array(list));
    Value::Object(content)
}

/// The emoji in `content`, most recently used first. Which of them PingCraft can show is the interface's call.
pub(crate) fn emoji(content: &Value) -> Vec<String> {
    content
        .get("recent_emoji")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter(|e| well_formed(e)).filter_map(|e| e.get("emoji")?.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// Uses on this device not yet sent to the server, and when to send them.
#[derive(Debug, Default)]
pub struct Pending {
    uses: Vec<String>,
    /// When the next attempt may be made: [`QUIET`] after the last use, or [`RETRY`] after a failure.
    not_before: Option<Instant>,
}

impl Pending {
    /// Notes a use, and holds the sending back until [`QUIET`] has passed without another.
    pub fn used(&mut self, emoji: String, now: Instant) {
        self.uses.push(emoji);
        self.not_before = Some(now + QUIET);
    }

    /// Whether there are uses to send and it is time to (`now`), or (`now` = `None`) whether there are any at all.
    pub fn due(&self, now: Option<Instant>) -> bool {
        !self.uses.is_empty() && now.is_none_or(|now| self.not_before.is_none_or(|at| now >= at))
    }

    /// The uses not sent yet, oldest first.
    pub fn uses(&self) -> &[String] {
        &self.uses
    }

    /// The uses were sent: forget them.
    fn sent(&mut self) {
        self.uses.clear();
        self.not_before = None;
    }

    /// Sending them failed: try again later.
    fn failed(&mut self, now: Instant) {
        self.not_before = Some(now + RETRY);
    }
}

fn problem(e: impl std::fmt::Display) -> String {
    format!("the recently used emoji could not be saved: {e}")
}

impl Connection {
    /// The account's recently used emoji as this device last synced them (no request to the server), with the uses
    /// not sent yet on top, most recent first. With the raw content, to tell when it changes.
    pub async fn recent_emoji(&self, pending: &Pending) -> (Vec<String>, String) {
        let account = self.client().account();
        let read = |kind: &str| {
            let account = account.clone();
            let kind = GlobalAccountDataEventType::from(kind);
            async move { account.account_data_raw(kind).await.ok().flatten().and_then(|raw| raw.deserialize_as_unchecked::<Value>().ok()) }
        };
        let (current, legacy) = (read(RECENT_EMOJI).await, read(LEGACY_RECENT_EMOJI).await);
        // Only compared, never shown or logged.
        let seen =
            format!("{}\n{}", current.as_ref().map(Value::to_string).unwrap_or_default(), legacy.as_ref().map(Value::to_string).unwrap_or_default());
        (emoji(&with_uses(base(current, legacy), pending.uses())), seen)
    }

    /// Sends the pending uses, on top of what the server has now, and returns the list as it then is. On failure
    /// the uses stay pending, to try again later.
    pub async fn save_recent_emoji(&self, pending: &mut Pending) -> Result<Vec<String>, String> {
        let account = self.client().account();
        let fetch = |kind: &str| {
            let account = account.clone();
            let kind = GlobalAccountDataEventType::from(kind);
            async move { account.fetch_account_data(kind).await.map(|raw| raw.and_then(|raw| raw.deserialize_as_unchecked::<Value>().ok())) }
        };
        let fetched = async { Ok::<_, matrix_sdk::Error>((fetch(RECENT_EMOJI).await?, fetch(LEGACY_RECENT_EMOJI).await?)) }.await;
        let (current, legacy) = match fetched {
            Ok(both) => both,
            Err(e) => {
                pending.failed(Instant::now());
                return Err(problem(e));
            }
        };
        let content = with_uses(base(current, legacy), pending.uses());
        let raw = Raw::new(&content).map_err(problem)?.cast_unchecked();
        if let Err(e) = account.set_account_data_raw(GlobalAccountDataEventType::from(RECENT_EMOJI), raw).await {
            pending.failed(Instant::now());
            return Err(problem(e));
        }
        pending.sent();
        Ok(emoji(&content))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::{LEGACY_RECENT_EMOJI, MAX, Pending, QUIET, RECENT_EMOJI, base, emoji, with_uses};

    fn uses(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    /// The names other Matrix apps read: the specification's, and Element's older one.
    #[test]
    fn the_event_types_are_the_shared_ones() {
        assert_eq!(RECENT_EMOJI, "m.recent_emoji");
        assert_eq!(LEGACY_RECENT_EMOJI, "io.element.recent_emoji");
        assert_eq!(MAX, 100);
    }

    #[test]
    fn a_use_moves_the_emoji_to_the_front_and_counts_it() {
        let content = json!({ "recent_emoji": [{ "emoji": "😎", "total": 1 }, { "emoji": "🏠", "total": 5 }] });
        let after = with_uses(content, &uses(&["🏠", "🎉"]));
        assert_eq!(after, json!({ "recent_emoji": [{ "emoji": "🎉", "total": 1 }, { "emoji": "🏠", "total": 6 }, { "emoji": "😎", "total": 1 }] }));
        assert_eq!(emoji(&after), ["🎉", "🏠", "😎"]);
    }

    /// Another device's change since this one last looked is kept: the uses go on top of what the server has.
    #[test]
    fn uses_are_merged_into_another_devices_list_not_over_it() {
        let server_now = json!({ "recent_emoji": [{ "emoji": "🚀", "total": 3 }, { "emoji": "👍", "total": 2 }] });
        let after = with_uses(server_now, &uses(&["👍"]));
        assert_eq!(emoji(&after), ["👍", "🚀"]);
        assert_eq!(after["recent_emoji"][0]["total"], 3);
        assert_eq!(after["recent_emoji"][1]["total"], 3);
    }

    #[test]
    fn unknown_entries_and_keys_are_kept_and_malformed_ones_dropped() {
        let content = json!({
            "recent_emoji": [
                { "emoji": ":custom:", "total": 4, "x.future": true },
                { "emoji": "🏠", "total": -1 },
                { "total": 2 },
                "nonsense",
            ],
            "x.other": 1,
        });
        let after = with_uses(content, &uses(&["🎉"]));
        assert_eq!(
            after,
            json!({
                "recent_emoji": [{ "emoji": "🎉", "total": 1 }, { "emoji": ":custom:", "total": 4, "x.future": true }],
                "x.other": 1,
            })
        );
    }

    #[test]
    fn the_list_is_cut_from_its_end() {
        let list: Vec<_> = (0..MAX + 10).map(|n| json!({ "emoji": format!("e{n}"), "total": 1 })).collect();
        let after = with_uses(json!({ "recent_emoji": list }), &uses(&["🎉"]));
        let shown = emoji(&after);
        assert_eq!(shown.len(), MAX);
        assert_eq!(shown[0], "🎉");
        assert_eq!(shown[MAX - 1], format!("e{}", MAX - 2));
    }

    #[test]
    fn an_account_with_only_elements_older_event_starts_from_it() {
        let legacy = json!({ "recent_emoji": [["😁", 2], ["🎉", 10], ["bad"]] });
        assert_eq!(base(None, Some(legacy.clone())), json!({ "recent_emoji": [{ "emoji": "😁", "total": 2 }, { "emoji": "🎉", "total": 10 }] }));
        let current = json!({ "recent_emoji": [{ "emoji": "👍", "total": 1 }] });
        assert_eq!(base(Some(current.clone()), Some(legacy)), current, "the specification's event wins once it exists");
        assert_eq!(base(None, None), json!({ "recent_emoji": [] }));
    }

    #[test]
    fn uses_wait_for_a_quiet_moment_and_retry_later_after_a_failure() {
        let start = Instant::now();
        let mut pending = Pending::default();
        assert!(!pending.due(None));
        pending.used("👍".to_owned(), start);
        pending.used("🎉".to_owned(), start + Duration::from_secs(2));
        assert!(pending.due(None), "there is something to send when PingCraft closes");
        assert!(!pending.due(Some(start + Duration::from_secs(3))), "still in the burst");
        assert!(pending.due(Some(start + Duration::from_secs(2) + QUIET)));
        pending.failed(start + Duration::from_secs(6));
        assert!(!pending.due(Some(start + Duration::from_secs(7))));
        assert!(pending.due(Some(start + Duration::from_secs(40))));
        assert_eq!(pending.uses(), ["👍", "🎉"]);
        pending.sent();
        assert!(!pending.due(None));
    }
}

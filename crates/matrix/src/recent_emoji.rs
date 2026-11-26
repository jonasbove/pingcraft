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

use matrix_sdk::config::RequestConfig;
use matrix_sdk::ruma::api::client::config::{get_global_account_data, set_global_account_data};
use matrix_sdk::ruma::api::error::ErrorKind;
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
/// How long one request may take, without retrying: sending happens on the engine's loop, which must not stall
/// (nor the window's closing wait) on a network that does not answer.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// How long sending may take in all (two reads and a write).
pub const SAVE_TIMEOUT: Duration = Duration::from_secs(12);

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
    /// What was last saved, and the synced copy as it was then: shown instead of that copy until the sync brings
    /// the saved one, so emoji just sent do not drop out of the list for a moment.
    saved: Option<(Value, String)>,
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

    /// The uses were sent (`content` is what the server now has, `seen` the synced copy then): forget them.
    fn sent(&mut self, content: Value, seen: String) {
        self.uses.clear();
        self.not_before = None;
        self.saved = Some((content, seen));
    }

    /// Sending them failed (or took too long): try again later.
    fn failed(&mut self, now: Instant) {
        self.not_before = Some(now + RETRY);
    }
}

fn problem(e: impl std::fmt::Display) -> String {
    format!("the recently used emoji could not be saved: {e}")
}

/// The synced copies of both events, and a text that changes when either does (compared, never shown or logged).
async fn stored(account: &matrix_sdk::Account) -> (Option<Value>, Option<Value>, String) {
    let read = |kind: &'static str| {
        let account = account.clone();
        async move {
            account
                .account_data_raw(GlobalAccountDataEventType::from(kind))
                .await
                .ok()
                .flatten()
                .and_then(|raw| raw.deserialize_as_unchecked::<Value>().ok())
        }
    };
    let (current, legacy) = (read(RECENT_EMOJI).await, read(LEGACY_RECENT_EMOJI).await);
    let text = |v: &Option<Value>| v.as_ref().map(Value::to_string).unwrap_or_default();
    let seen = format!("{}\n{}", text(&current), text(&legacy));
    (current, legacy, seen)
}

/// The list to show: the synced copy (or, until the sync brings it, what this device last saved) with the uses not
/// sent yet on top.
fn shown(current: Option<Value>, legacy: Option<Value>, seen: &str, pending: &Pending) -> Vec<String> {
    let synced = match &pending.saved {
        Some((saved, then)) if then == seen => saved.clone(),
        _ => base(current, legacy),
    };
    emoji(&with_uses(synced, pending.uses()))
}

/// One request's settings: a short timeout and no retries (the next attempt is [`RETRY`] later).
fn once() -> RequestConfig {
    RequestConfig::new().timeout(REQUEST_TIMEOUT).disable_retry()
}

impl Connection {
    /// The account's recently used emoji as this device last synced them (no request to the server), with the uses
    /// not sent yet on top, most recent first. With a text that changes when the synced copy does.
    pub async fn recent_emoji(&self, pending: &Pending) -> (Vec<String>, String) {
        let (current, legacy, seen) = stored(&self.client().account()).await;
        (shown(current, legacy, &seen, pending), seen)
    }

    /// The server's copy of one event, asked for now (not the synced one). `None` when the account has none.
    async fn fetch(&self, kind: &str) -> Result<Option<Value>, matrix_sdk::HttpError> {
        let Some(user) = self.client().user_id() else { return Ok(None) };
        let request = get_global_account_data::v3::Request::new(user.to_owned(), GlobalAccountDataEventType::from(kind));
        match self.client().send(request).with_request_config(once()).await {
            Ok(response) => Ok(response.account_data.deserialize_as_unchecked::<Value>().ok()),
            Err(e) if e.client_api_error_kind() == Some(&ErrorKind::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Sends the pending uses, on top of what the server has now, and returns the list as it then is. On failure,
    /// or after [`SAVE_TIMEOUT`], the uses stay pending, to try again later.
    pub async fn save_recent_emoji(&self, pending: &mut Pending) -> Result<Vec<String>, String> {
        let done = tokio::time::timeout(SAVE_TIMEOUT, self.try_save(pending.uses())).await;
        match done {
            Ok(Ok((content, seen))) => {
                let list = emoji(&content);
                pending.sent(content, seen);
                Ok(list)
            }
            Ok(Err(e)) => {
                pending.failed(Instant::now());
                Err(e)
            }
            Err(_) => {
                pending.failed(Instant::now());
                Err(problem("the server took too long"))
            }
        }
    }

    async fn try_save(&self, uses: &[String]) -> Result<(Value, String), String> {
        let current = self.fetch(RECENT_EMOJI).await.map_err(problem)?;
        let legacy = if current.is_some() { None } else { self.fetch(LEGACY_RECENT_EMOJI).await.map_err(problem)? };
        let content = with_uses(base(current, legacy), uses);
        let Some(user) = self.client().user_id() else { return Err(problem("not signed in")) };
        let raw = Raw::new(&content).map_err(problem)?.cast_unchecked();
        let (_, _, seen) = stored(&self.client().account()).await;
        let request = set_global_account_data::v3::Request::new_raw(user.to_owned(), GlobalAccountDataEventType::from(RECENT_EMOJI), raw);
        self.client().send(request).with_request_config(once()).await.map_err(problem)?;
        Ok((content, seen))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::{LEGACY_RECENT_EMOJI, MAX, Pending, QUIET, RECENT_EMOJI, SAVE_TIMEOUT, base, emoji, shown, with_uses};

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
        pending.sent(json!({ "recent_emoji": [] }), String::new());
        assert!(!pending.due(None));
    }

    /// Emoji just sent stay in the list until the sync brings the server's copy, also when another is used
    /// meanwhile; once the synced copy changes, it is shown.
    #[test]
    fn what_was_just_saved_is_shown_until_the_sync_brings_it() {
        let old = Some(json!({ "recent_emoji": [{ "emoji": "🚀", "total": 1 }] }));
        let mut pending = Pending::default();
        pending.sent(json!({ "recent_emoji": [{ "emoji": "👍", "total": 1 }, { "emoji": "🚀", "total": 1 }] }), "before".into());
        pending.used("🎉".to_owned(), Instant::now());
        assert_eq!(shown(old.clone(), None, "before", &pending), ["🎉", "👍", "🚀"], "the sync has not brought it yet");
        let new = Some(json!({ "recent_emoji": [{ "emoji": "👍", "total": 1 }, { "emoji": "🚀", "total": 1 }] }));
        assert_eq!(shown(new, None, "after", &pending), ["🎉", "👍", "🚀"]);
        assert_eq!(shown(old, None, "changed elsewhere", &pending), ["🎉", "🚀"], "the synced copy wins once it changes");
    }

    /// Sending happens on the engine's loop and while the window closes: it gives up rather than hang.
    #[test]
    fn saving_gives_up_rather_than_hang() {
        assert!(SAVE_TIMEOUT <= Duration::from_secs(15));
    }
}

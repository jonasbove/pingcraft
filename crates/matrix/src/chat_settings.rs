//! A chat's own settings: pinned to the top of the inbox (the `m.favourite` tag), muted (a push rule, with the time
//! it ends in PingCraft's own room account data), marked unread (`m.marked_unread`), archived (when, in PingCraft's
//! own room account data), and the user's draft (in the SDK's encrypted state store on this device, never in a plain
//! file: ADR 0004).
//!
//! Archiving is not the `m.lowpriority` tag: other Matrix apps show that as a "low priority" section, not as out of
//! the inbox, the SDK takes a chat's `m.favourite` (its pin) off when it sets it, and a tag cannot say when the chat
//! was archived, which PingCraft needs to bring it back when a newer message arrives.
//!
//! Each change goes to the server, which keeps it for the user's other Matrix apps too. The inbox learns of it with
//! the next sync; each call also returns the chat as it now is, so the interface does not wait for that.

use matrix_sdk::Room;
use matrix_sdk::notification_settings::RoomNotificationMode;
use matrix_sdk::ruma::OwnedEventId;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::RoomAccountDataEventType;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::store::{ComposerDraft, ComposerDraftType};
use pingcraft_core::{Accent, Alerts, Chat, ChatId, DraftFor, Mute, Timestamp};
use serde_json::json;

use crate::connection::Connection;
use crate::inbox::chat_from_room;

/// Room account data where PingCraft keeps when a mute ends. Push rules have no end time; PingCraft turns the
/// notifications back on itself, while it runs.
pub(crate) const MUTE_UNTIL: &str = crate::namespaced!("mute_until");

/// When a chat's mute ends, from its room account data: `None` when PingCraft set no end.
pub(crate) async fn mute_until(room: &Room) -> Option<Timestamp> {
    let raw = room.account_data(RoomAccountDataEventType::from(MUTE_UNTIL)).await.ok()??;
    let content: serde_json::Value = raw.get_field("content").ok()??;
    content.get("until").and_then(serde_json::Value::as_u64).filter(|&t| t > 0).map(Timestamp)
}

/// Room account data where PingCraft keeps a chat's own notification sound and accent colour
/// (`{"sound": true, "accent": "#1c56c4"}`, each key left out to follow the network's). PingCraft's own: no Matrix
/// event says how a client should sound or colour a chat. The colour is any `#rrggbb` (maintainer, 2026-11-26); data
/// saved before then holds a network's id instead (`"signal"`), which still reads as that network's colour.
pub(crate) const ALERTS: &str = crate::namespaced!("alerts");

/// Alerts from saved content: only a true or false sound and a colour [`Accent::parse`] reads are taken; anything
/// else is unset.
pub(crate) fn alerts_from(content: &serde_json::Value) -> Alerts {
    Alerts {
        sound: content.get("sound").and_then(serde_json::Value::as_bool),
        accent: content.get("accent").and_then(serde_json::Value::as_str).and_then(Accent::parse),
    }
}

/// Alerts as saved: unset ones are left out.
pub(crate) fn alerts_json(alerts: Alerts) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(sound) = alerts.sound {
        map.insert("sound".to_owned(), json!(sound));
    }
    if let Some(accent) = alerts.accent {
        map.insert("accent".to_owned(), json!(accent.hex()));
    }
    serde_json::Value::Object(map)
}

/// A chat's own alerts, from its room account data.
pub(crate) async fn alerts(room: &Room) -> Alerts {
    let Some(raw) = room.account_data(RoomAccountDataEventType::from(ALERTS)).await.ok().flatten() else { return Alerts::default() };
    raw.get_field::<serde_json::Value>("content").ok().flatten().map(|c| alerts_from(&c)).unwrap_or_default()
}

/// Room account data where PingCraft keeps when the user archived a chat (`{"at": ms}`; 0 when it is not).
pub(crate) const ARCHIVED: &str = crate::namespaced!("archived");

/// When the user archived a chat, from its room account data: `None` when it is not archived.
pub(crate) async fn archived(room: &Room) -> Option<Timestamp> {
    let raw = room.account_data(RoomAccountDataEventType::from(ARCHIVED)).await.ok()??;
    let content: serde_json::Value = raw.get_field("content").ok()??;
    content.get("at").and_then(serde_json::Value::as_u64).filter(|&t| t > 0).map(Timestamp)
}

/// The time to archive a chat at: now, or its last message's time if that is later (another server's clock may run
/// ahead of this computer's, and the chat must not come straight back).
pub(crate) fn archive_time(now: Timestamp, last_message: Option<Timestamp>) -> Timestamp {
    now.max(last_message.unwrap_or_default())
}

/// This computer's clock, in milliseconds since 1970 (as the server's timestamps are), through ruma, which also
/// reads the browser's clock on the web.
fn now_ms() -> Timestamp {
    Timestamp(u64::from(matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now().get()))
}

fn problem(what: &str, e: impl std::fmt::Display) -> String {
    format!("{what}: {e}")
}

impl Connection {
    fn room(&self, chat: &ChatId) -> Result<Room, String> {
        let id = OwnedRoomId::try_from(chat.0.as_str()).map_err(|_| "no such chat".to_owned())?;
        self.client().get_room(&id).ok_or_else(|| "no such chat".to_owned())
    }

    async fn chat_now(&self, room: &Room) -> Chat {
        chat_from_room(room, self.client().user_id()).await
    }

    /// Pins a chat to the top of the inbox, or unpins it.
    pub async fn pin_chat(&self, chat: &ChatId, pin: bool) -> Result<Chat, String> {
        let room = self.room(chat)?;
        room.set_is_favourite(pin, None).await.map_err(|e| problem("the chat could not be pinned", e))?;
        let mut now = self.chat_now(&room).await;
        now.pinned = pin;
        Ok(now)
    }

    /// Mutes a chat, or (`None`) turns its notifications back on.
    pub async fn mute(&self, chat: &ChatId, mute: Option<Mute>) -> Result<Chat, String> {
        let room = self.room(chat)?;
        let settings = self.client().notification_settings().await;
        let until = match mute {
            Some(Mute::Until(at)) => at.0,
            _ => 0,
        };
        // The end time first: a mute without one is a mute for good, which a failure here must not leave behind.
        set_mute_until(&room, until).await?;
        match mute {
            Some(_) => settings
                .set_room_notification_mode(room.room_id(), RoomNotificationMode::Mute)
                .await
                .map_err(|e| problem("the chat's notifications could not be changed", e))?,
            None => self.delete_room_rules(&room).await?,
        }
        let mut now = self.chat_now(&room).await;
        now.muted = mute;
        Ok(now)
    }

    /// Takes away the chat's own push rules on the server, whatever this device has synced of them. The SDK's
    /// `delete_user_defined_room_rules` deletes only the rules this device's synced copy holds: right after a mute
    /// (here or elsewhere), before the sync brings its rule back, it found none and the chat stayed muted on the
    /// server. Each kind of rule a chat can have is deleted by its ID (the room's), and "not there" is fine.
    async fn delete_room_rules(&self, room: &Room) -> Result<(), String> {
        use matrix_sdk::ruma::api::client::push::delete_pushrule;
        use matrix_sdk::ruma::api::error::ErrorKind;
        for kind in room_rule_kinds() {
            let request = delete_pushrule::v3::Request::new(kind, room.room_id().to_string());
            match self.client().send(request).await {
                Ok(_) => {}
                Err(e) if e.client_api_error_kind() == Some(&ErrorKind::NotFound) => {}
                Err(e) => return Err(problem("the chat's notifications could not be changed", e)),
            }
        }
        Ok(())
    }

    /// Marks a chat as unread.
    pub async fn mark_unread(&self, chat: &ChatId) -> Result<Chat, String> {
        let room = self.room(chat)?;
        room.set_unread_flag(true).await.map_err(|e| problem("the chat could not be marked unread", e))?;
        let mut now = self.chat_now(&room).await;
        now.marked_unread = true;
        Ok(now)
    }

    /// Moves a chat into the archive, or back into the inbox.
    pub async fn archive(&self, chat: &ChatId, archive: bool) -> Result<Chat, String> {
        let room = self.room(chat)?;
        let mut now = self.chat_now(&room).await;
        let at = archive.then(|| archive_time(now_ms(), now.last_message.as_ref().map(|m| m.at)));
        let content = Raw::new(&json!({ "at": at.map_or(0, |t| t.0) })).map_err(|e| problem("the chat could not be archived", e))?.cast_unchecked();
        room.set_account_data_raw(RoomAccountDataEventType::from(ARCHIVED), content)
            .await
            .map_err(|e| problem(if archive { "the chat could not be archived" } else { "the chat could not be moved back to the inbox" }, e))?;
        now.archived = at;
        Ok(now)
    }

    /// Sets a chat's own notification sound and accent colour.
    pub async fn set_chat_alerts(&self, chat: &ChatId, alerts: Alerts) -> Result<Chat, String> {
        let room = self.room(chat)?;
        let content = Raw::new(&alerts_json(alerts)).map_err(|e| problem("the chat's sound and colour could not be saved", e))?.cast_unchecked();
        room.set_account_data_raw(RoomAccountDataEventType::from(ALERTS), content)
            .await
            .map_err(|e| problem("the chat's sound and colour could not be saved", e))?;
        let mut now = self.chat_now(&room).await;
        now.alerts = alerts;
        Ok(now)
    }

    /// Keeps the user's draft for a chat, with the message it replies to or edits (by event ID), or (empty) forgets
    /// it.
    pub async fn save_draft(&self, chat: &ChatId, text: &str, draft_for: Option<&DraftFor>) -> Result<Chat, String> {
        let room = self.room(chat)?;
        let text = text.trim();
        let done = if text.is_empty() {
            room.clear_composer_draft(None).await
        } else {
            let draft = ComposerDraft { plain_text: text.to_owned(), html_text: None, draft_type: draft_type(draft_for), attachments: Vec::new() };
            room.save_composer_draft(draft, None).await
        };
        done.map_err(|e| problem("the draft could not be kept", e))?;
        let mut now = self.chat_now(&room).await;
        now.draft = (!text.is_empty()).then(|| text.to_owned());
        now.draft_for = if text.is_empty() { None } else { draft_for.cloned().filter(|f| draft_type(Some(f)) != ComposerDraftType::NewMessage) };
        Ok(now)
    }

    /// Tells a chat whether the user is typing.
    pub async fn typing(&self, chat: &ChatId, typing: bool) -> Result<(), String> {
        self.room(chat)?.typing_notice(typing).await.map_err(|e| problem("could not tell the chat you are typing", e))
    }

    /// Takes the "marked unread" off a chat that was opened. Harmless when it has none.
    pub async fn opened(&self, chat: &ChatId) -> Result<Option<Chat>, String> {
        let room = self.room(chat)?;
        if !room.is_marked_unread() {
            return Ok(None);
        }
        room.set_unread_flag(false).await.map_err(|e| problem("the chat could not be marked read", e))?;
        let mut now = self.chat_now(&room).await;
        now.marked_unread = false;
        Ok(Some(now))
    }
}

async fn set_mute_until(room: &Room, until: u64) -> Result<(), String> {
    let content = Raw::new(&json!({ "until": until })).map_err(|e| problem("the mute could not be saved", e))?.cast_unchecked();
    room.set_account_data_raw(RoomAccountDataEventType::from(MUTE_UNTIL), content)
        .await
        .map(|_| ())
        .map_err(|e| problem("the mute could not be saved", e))
}

/// The SDK's kind of draft for what a draft is for. A message ID that is not an event ID (one still being sent)
/// makes a new message's draft.
fn draft_type(draft_for: Option<&DraftFor>) -> ComposerDraftType {
    let event = |id: &String| OwnedEventId::try_from(id.as_str()).ok();
    match draft_for {
        Some(DraftFor::Reply(id)) => event(id).map_or(ComposerDraftType::NewMessage, |event_id| ComposerDraftType::Reply { event_id }),
        Some(DraftFor::Edit(id)) => event(id).map_or(ComposerDraftType::NewMessage, |event_id| ComposerDraftType::Edit { event_id }),
        None => ComposerDraftType::NewMessage,
    }
}

/// What a draft of the SDK's kind is for.
fn draft_for(kind: ComposerDraftType) -> Option<DraftFor> {
    match kind {
        ComposerDraftType::Reply { event_id } => Some(DraftFor::Reply(event_id.to_string())),
        ComposerDraftType::Edit { event_id } => Some(DraftFor::Edit(event_id.to_string())),
        ComposerDraftType::NewMessage => None,
    }
}

/// The user's draft for a room, from the store, and what it is for.
pub(crate) async fn draft(room: &Room) -> (Option<String>, Option<DraftFor>) {
    match room.load_composer_draft(None).await.ok().flatten().filter(|d| !d.plain_text.trim().is_empty()) {
        Some(d) => (Some(d.plain_text), draft_for(d.draft_type)),
        None => (None, None),
    }
}

/// Whether a room is muted, by the rule the sync last reported.
/// The kinds of push rule a chat's own settings use, each with the room's ID as its rule ID: a mute is an override
/// rule, "mentions only" and the like are room rules.
fn room_rule_kinds() -> [matrix_sdk::ruma::push::RuleKind; 2] {
    use matrix_sdk::ruma::push::RuleKind;
    [RuleKind::Override, RuleKind::Room]
}

pub(crate) fn is_muted(room: &Room) -> bool {
    room.cached_user_defined_notification_mode() == Some(RoomNotificationMode::Mute)
}

#[cfg(test)]
mod tests {
    use pingcraft_core::Timestamp;

    use matrix_sdk::store::ComposerDraftType;
    use pingcraft_core::DraftFor;

    use pingcraft_core::{Accent, Alerts};
    use serde_json::json;

    use super::{ALERTS, ARCHIVED, MUTE_UNTIL, alerts_from, alerts_json, archive_time, draft_for, draft_type};

    /// What is saved reads back the same, the colour as `#rrggbb`; anything PingCraft does not know is unset, never
    /// guessed. (Before any colour could be chosen, 2026-11-26, a `#` colour was refused; now it is the format.)
    #[test]
    fn alerts_are_saved_and_read_back_and_nothing_else_is_taken() {
        assert_eq!(ALERTS, "io.github.jonasbove.pingcraft.alerts");
        let mine = Accent([0xff, 0x00, 0xff]);
        for alerts in [Alerts::default(), Alerts { sound: Some(false), accent: Some(mine) }, Alerts { sound: Some(true), accent: None }] {
            assert_eq!(alerts_from(&alerts_json(alerts)), alerts);
        }
        assert_eq!(alerts_json(Alerts { sound: None, accent: Some(mine) }), json!({ "accent": "#ff00ff" }));
        assert_eq!(alerts_json(Alerts::default()), json!({}));
        assert_eq!(alerts_from(&json!({ "sound": "yes", "accent": "fuchsia" })), Alerts::default());
        assert_eq!(alerts_from(&json!({ "accent": 0xff00ff })), Alerts::default());
        assert_eq!(alerts_from(&json!([1])), Alerts::default());
    }

    /// A chat's colour saved before any colour could be chosen holds a network's id: it reads as that network's
    /// colour, and is saved as `#rrggbb` the next time the chat's settings are written.
    #[test]
    fn a_colour_saved_as_a_network_id_still_reads() {
        let old = json!({ "sound": false, "accent": "discord" });
        let read = alerts_from(&old);
        assert_eq!(read, Alerts { sound: Some(false), accent: Accent::of_network("discord") });
        assert_eq!(read.accent.and_then(Accent::network), Some("discord"));
        let again = alerts_json(read);
        assert!(again["accent"].as_str().is_some_and(|hex| hex.starts_with('#') && hex.len() == 7), "{again}");
        assert_eq!(alerts_from(&again), read);
    }

    /// The name other Matrix apps and later versions find the data under: it must not change by accident.
    #[test]
    fn mute_until_is_in_pingcraft_namespace() {
        assert_eq!(crate::NAMESPACE, "io.github.jonasbove.pingcraft");
        assert_eq!(MUTE_UNTIL, "io.github.jonasbove.pingcraft.mute_until");
        assert!(MUTE_UNTIL.starts_with(&format!("{}.", crate::NAMESPACE)));
        assert_eq!(ARCHIVED, "io.github.jonasbove.pingcraft.archived");
    }

    /// A draft's reply or edit goes into the store and comes back as it was; one for a message with no event ID yet
    /// is kept as a new message's draft.
    #[test]
    fn what_a_draft_is_for_is_kept() {
        for kept in [DraftFor::Reply("$abc:example.org".into()), DraftFor::Edit("$def:example.org".into())] {
            assert_eq!(draft_for(draft_type(Some(&kept))), Some(kept));
        }
        assert_eq!(draft_type(Some(&DraftFor::Reply("not an event ID".into()))), ComposerDraftType::NewMessage);
        assert_eq!(draft_for(draft_type(None)), None);
    }

    /// A chat whose last message is stamped ahead of this computer's clock is archived at that time, so it does not
    /// come straight back.
    #[test]
    fn a_chat_is_archived_after_its_last_message() {
        assert_eq!(archive_time(Timestamp(100), Some(Timestamp(40))), Timestamp(100));
        assert_eq!(archive_time(Timestamp(100), Some(Timestamp(140))), Timestamp(140));
        assert_eq!(archive_time(Timestamp(100), None), Timestamp(100));
    }
}

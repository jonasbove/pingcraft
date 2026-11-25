//! A chat's own settings: pinned to the top of the inbox (the `m.favourite` tag), muted (a push rule, with the time
//! it ends in PingCraft's own room account data), marked unread (`m.marked_unread`), and the user's draft (in the
//! SDK's encrypted state store on this device, never in a plain file: ADR 0004).
//!
//! Each change goes to the server, which keeps it for the user's other Matrix apps too. The inbox learns of it with
//! the next sync; each call also returns the chat as it now is, so the interface does not wait for that.

use matrix_sdk::Room;
use matrix_sdk::notification_settings::RoomNotificationMode;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::RoomAccountDataEventType;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::store::{ComposerDraft, ComposerDraftType};
use pingcraft_core::{Chat, ChatId, Mute, Timestamp};
use serde_json::json;

use crate::connection::Connection;
use crate::inbox::chat_from_room;

/// Room account data where PingCraft keeps when a mute ends. Push rules have no end time; PingCraft turns the
/// notifications back on itself, while it runs.
pub(crate) const MUTE_UNTIL: &str = "org.pingcraft.mute_until";

/// When a chat's mute ends, from its room account data: `None` when PingCraft set no end.
pub(crate) async fn mute_until(room: &Room) -> Option<Timestamp> {
    let raw = room.account_data(RoomAccountDataEventType::from(MUTE_UNTIL)).await.ok()??;
    let content: serde_json::Value = raw.get_field("content").ok()??;
    content.get("until").and_then(serde_json::Value::as_u64).filter(|&t| t > 0).map(Timestamp)
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
            Some(_) => settings.set_room_notification_mode(room.room_id(), RoomNotificationMode::Mute).await,
            None => settings.delete_user_defined_room_rules(room.room_id()).await,
        }
        .map_err(|e| problem("the chat's notifications could not be changed", e))?;
        let mut now = self.chat_now(&room).await;
        now.muted = mute;
        Ok(now)
    }

    /// Marks a chat as unread.
    pub async fn mark_unread(&self, chat: &ChatId) -> Result<Chat, String> {
        let room = self.room(chat)?;
        room.set_unread_flag(true).await.map_err(|e| problem("the chat could not be marked unread", e))?;
        let mut now = self.chat_now(&room).await;
        now.marked_unread = true;
        Ok(now)
    }

    /// Keeps the user's draft for a chat, or (empty) forgets it.
    pub async fn save_draft(&self, chat: &ChatId, text: &str) -> Result<Chat, String> {
        let room = self.room(chat)?;
        let text = text.trim();
        let done = if text.is_empty() {
            room.clear_composer_draft(None).await
        } else {
            let draft =
                ComposerDraft { plain_text: text.to_owned(), html_text: None, draft_type: ComposerDraftType::NewMessage, attachments: Vec::new() };
            room.save_composer_draft(draft, None).await
        };
        done.map_err(|e| problem("the draft could not be kept", e))?;
        let mut now = self.chat_now(&room).await;
        now.draft = (!text.is_empty()).then(|| text.to_owned());
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

/// The user's draft for a room, from the store.
pub(crate) async fn draft(room: &Room) -> Option<String> {
    room.load_composer_draft(None).await.ok().flatten().map(|d| d.plain_text).filter(|t| !t.trim().is_empty())
}

/// Whether a room is muted, by the rule the sync last reported.
pub(crate) fn is_muted(room: &Room) -> bool {
    room.cached_user_defined_notification_mode() == Some(RoomNotificationMode::Mute)
}

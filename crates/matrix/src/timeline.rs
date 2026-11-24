//! One open chat: `matrix-sdk-ui`'s `Timeline`, mapped onto [`pingcraft_core::Message`].
//!
//! The interface gets the chat's messages as a whole list each time something changes (a new message, a local
//! echo turning into a sent message, an older page arriving). Timelines are short enough on screen that this is
//! simpler and safer than replaying diffs, and it keeps SDK types out of the interface.

use std::pin::pin;
use std::sync::Arc;

use futures_util::StreamExt;
use matrix_sdk::executor::{AbortOnDrop, spawn};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::{MessageType, RoomMessageEventContent};
use matrix_sdk_ui::Timeline;
use matrix_sdk_ui::eyeball_im::Vector;
use matrix_sdk_ui::timeline::{
    EventSendState, EventTimelineItem, MsgLikeKind, RoomExt, TimelineDetails, TimelineItem, TimelineItemContent, TimelineUniqueId,
};
use pingcraft_core::{ChatId, Message, MessageBody, MessageId, SendState, Timestamp};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::connection::Connection;

/// Why a chat could not be opened or a message could not be sent, in the SDK's words, for the log.
#[derive(Debug)]
pub enum TimelineError {
    /// The chat is not (or no longer) in the inbox.
    UnknownChat,
    /// The message to retry is not there, or was not a failed one.
    NothingToRetry,
    /// Anything the SDK reported.
    Sdk(String),
}

impl std::fmt::Display for TimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownChat => f.write_str("that chat is not in the inbox"),
            Self::NothingToRetry => f.write_str("there is no failed message to send again"),
            Self::Sdk(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for TimelineError {}

fn sdk(e: impl std::fmt::Display) -> TimelineError {
    TimelineError::Sdk(e.to_string())
}

/// An open chat. Its messages arrive as whole lists, oldest first; dropping it closes the chat.
pub struct ChatTimeline {
    timeline: Timeline,
    connection_rooms: Arc<matrix_sdk_ui::RoomListService>,
    room_id: OwnedRoomId,
    lists: UnboundedReceiver<Vec<Message>>,
    _task: AbortOnDrop<()>,
}

impl Connection {
    /// Opens a chat from the inbox. The room is subscribed to, so it gets its full recent history and live
    /// updates, not just the one event the inbox asks for.
    pub async fn open_chat(&self, chat: &ChatId) -> Result<ChatTimeline, TimelineError> {
        let rooms = self.sync_service().room_list_service();
        let room_id = OwnedRoomId::try_from(chat.0.as_str()).map_err(|_| TimelineError::UnknownChat)?;
        let room = rooms.room(&room_id).map_err(|_| TimelineError::UnknownChat)?;
        rooms.set_room_subscriptions(&[&room_id]).await;
        let timeline = room.timeline().await.map_err(sdk)?;
        let (items, updates) = timeline.subscribe().await;
        let (tx, lists) = unbounded_channel();
        let task = spawn(async move {
            let mut items: Vector<Arc<TimelineItem>> = items;
            if tx.send(messages(&items)).is_err() {
                return;
            }
            let mut updates = pin!(updates);
            while let Some(diffs) = updates.next().await {
                for diff in diffs {
                    diff.apply(&mut items);
                }
                if tx.send(messages(&items)).is_err() {
                    return;
                }
            }
        });
        Ok(ChatTimeline { timeline, connection_rooms: rooms, room_id, lists, _task: AbortOnDrop::new(task) })
    }
}

impl ChatTimeline {
    /// The newest list of messages, if anything changed since the last call. Never blocks; skips lists that were
    /// already out of date.
    pub fn try_next(&mut self) -> Option<Vec<Message>> {
        let mut newest = None;
        while let Ok(list) = self.lists.try_recv() {
            newest = Some(list);
        }
        newest
    }

    /// Waits for the next list of messages. `None` when the chat was closed underneath it.
    pub async fn next(&mut self) -> Option<Vec<Message>> {
        self.lists.recv().await
    }

    /// Sends a text message. It shows at once as [`SendState::Sending`], then as sent or failed.
    pub async fn send_text(&self, text: &str) -> Result<(), TimelineError> {
        self.timeline.send(RoomMessageEventContent::text_plain(text).into()).await.map(|_| ()).map_err(sdk)
    }

    /// Sends a failed message again.
    pub async fn retry(&self, message: &MessageId) -> Result<(), TimelineError> {
        let items = self.timeline.items().await;
        let item = items
            .iter()
            .find(|i| i.unique_id() == &TimelineUniqueId(message.0.clone()))
            .and_then(|i| i.as_event().cloned())
            .ok_or(TimelineError::NothingToRetry)?;
        if !matches!(item.send_state(), Some(EventSendState::SendingFailed { .. })) {
            return Err(TimelineError::NothingToRetry);
        }
        let handle = item.local_echo_send_handle().ok_or(TimelineError::NothingToRetry)?;
        // An error the SDK cannot recover from parks the message ("wedged"); one it can pauses the chat's whole
        // queue. Undo both: harmless for the one that did not happen.
        handle.unwedge().await.map_err(sdk)?;
        self.timeline.room().send_queue().set_enabled(true);
        Ok(())
    }

    /// Asks for older messages. Returns whether the start of the chat has been reached.
    pub async fn load_older(&self, count: u16) -> Result<bool, TimelineError> {
        self.timeline.paginate_backwards(count).await.map_err(sdk)
    }

    /// Tells the server the user has read the chat up to its newest message.
    pub async fn mark_read(&self) -> Result<(), TimelineError> {
        self.timeline.mark_as_read(matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType::Read).await.map(|_| ()).map_err(sdk)
    }
}

impl Drop for ChatTimeline {
    fn drop(&mut self) {
        self.connection_rooms.remove_room_subscriptions(&[&self.room_id]);
    }
}

/// The messages among the timeline's items, oldest first. Date dividers, read markers and room state changes are
/// left out: milestone 2 shows messages only.
fn messages(items: &Vector<Arc<TimelineItem>>) -> Vec<Message> {
    items.iter().filter_map(|item| item.as_event().and_then(|event| message(item.unique_id(), event))).collect()
}

fn message(id: &TimelineUniqueId, event: &EventTimelineItem) -> Option<Message> {
    let TimelineItemContent::MsgLike(content) = event.content() else { return None };
    let body = match &content.kind {
        MsgLikeKind::Message(message) => body_of(message.msgtype()),
        MsgLikeKind::UnableToDecrypt(_) => MessageBody::WaitingForKey,
        MsgLikeKind::Redacted => MessageBody::Unsupported("deleted".into()),
        MsgLikeKind::Sticker(_) => MessageBody::Unsupported("sticker".into()),
        MsgLikeKind::Poll(_) => MessageBody::Unsupported("poll".into()),
        MsgLikeKind::LiveLocation(_) => MessageBody::Unsupported("location".into()),
        MsgLikeKind::Other(_) => MessageBody::Unsupported("message".into()),
    };
    let sender = match event.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.clone().unwrap_or_else(|| event.sender().to_string()),
        _ => event.sender().to_string(),
    };
    let state = match event.send_state() {
        None | Some(EventSendState::Sent { .. }) => SendState::Sent,
        Some(EventSendState::NotSentYet { .. }) => SendState::Sending,
        Some(EventSendState::SendingFailed { error, .. }) => SendState::Failed(error.to_string()),
    };
    Some(Message { id: MessageId(id.0.clone()), sender, own: event.is_own(), at: Timestamp(u64::from(event.timestamp().0)), body, state })
}

fn body_of(msgtype: &MessageType) -> MessageBody {
    match msgtype {
        MessageType::Text(t) => MessageBody::Text(t.body.clone()),
        MessageType::Notice(t) => MessageBody::Text(t.body.clone()),
        MessageType::Emote(t) => MessageBody::Text(t.body.clone()),
        MessageType::Image(_) => MessageBody::Unsupported("image".into()),
        MessageType::File(_) => MessageBody::Unsupported("file".into()),
        MessageType::Audio(_) => MessageBody::Unsupported("audio".into()),
        MessageType::Video(_) => MessageBody::Unsupported("video".into()),
        MessageType::Location(_) => MessageBody::Unsupported("location".into()),
        _ => MessageBody::Unsupported("message".into()),
    }
}

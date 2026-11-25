//! One open chat: `matrix-sdk-ui`'s `Timeline`, mapped onto [`pingcraft_core::Message`].
//!
//! The interface gets the chat's messages as a whole list each time something changes (a new message, a local
//! echo turning into a sent message, an older page arriving). Timelines are short enough on screen that this is
//! simpler and safer than replaying diffs, and it keeps SDK types out of the interface.

use std::collections::{HashMap, HashSet};
use std::pin::pin;
use std::sync::Arc;

use futures_util::StreamExt;
use matrix_sdk::executor::{AbortOnDrop, spawn};
use matrix_sdk::room::edit::EditedContent;
use matrix_sdk::ruma::events::room::message::{MessageType, RoomMessageEventContent, RoomMessageEventContentWithoutRelation};
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, UserId};
use matrix_sdk_ui::Timeline;
use matrix_sdk_ui::eyeball_im::Vector;
use matrix_sdk_ui::timeline::{
    EventSendState, EventTimelineItem, InReplyToDetails, MsgLikeContent, MsgLikeKind, Profile, RoomExt, TimelineDetails, TimelineItem,
    TimelineItemContent, TimelineUniqueId,
};
use pingcraft_core::{Abilities, ChatId, Message, MessageBody, MessageId, Quote, Reaction, SendState, Timestamp};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::connection::Connection;

/// Why a chat could not be opened or a message could not be sent, in the SDK's words, for the log.
#[derive(Debug)]
pub enum TimelineError {
    /// The chat is not (or no longer) in the inbox.
    UnknownChat,
    /// The message to retry is not there, or was not a failed one.
    NothingToRetry,
    /// The message is not in the chat's loaded messages (any more).
    NoSuchMessage,
    /// The message has not reached the server yet, so it cannot be replied to.
    NotSentYet,
    /// Anything the SDK reported.
    Sdk(String),
}

impl std::fmt::Display for TimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownChat => f.write_str("that chat is not in the inbox"),
            Self::NothingToRetry => f.write_str("there is no failed message to send again"),
            Self::NoSuchMessage => f.write_str("that message is no longer in the chat"),
            Self::NotSentYet => f.write_str("that message has not been sent yet"),
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
    timeline: Arc<Timeline>,
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
        // Shared with the task that fetches quoted messages. The SDK's timeline is Send and Sync on the desktop; in
        // the browser it is neither, and there everything runs on one thread anyway.
        #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
        let timeline = Arc::new(timeline);
        let (items, updates) = timeline.subscribe().await;
        let (tx, lists) = unbounded_channel();
        let own = room.own_user_id().to_owned();
        let fetcher = Arc::clone(&timeline);
        let task = spawn(async move {
            let mut items: Vector<Arc<TimelineItem>> = items;
            // Replies to messages that are not loaded: each original is asked for once.
            let mut asked: HashSet<OwnedEventId> = HashSet::new();
            let mut updates = pin!(updates);
            loop {
                let (list, missing) = messages(&items, &own);
                if tx.send(list).is_err() {
                    return;
                }
                for event in missing {
                    if asked.insert(event.clone()) {
                        // What it finds arrives as an update; failing leaves the quote as "loading".
                        let _ = fetcher.fetch_details_for_event(&event).await;
                    }
                }
                let Some(diffs) = updates.next().await else { return };
                for diff in diffs {
                    diff.apply(&mut items);
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

    /// The timeline's item for a message.
    async fn item(&self, message: &MessageId) -> Result<EventTimelineItem, TimelineError> {
        self.timeline
            .items()
            .await
            .iter()
            .find(|i| i.unique_id() == &TimelineUniqueId(message.0.clone()))
            .and_then(|i| i.as_event().cloned())
            .ok_or(TimelineError::NoSuchMessage)
    }

    /// Sends a text message that replies to `to`.
    pub async fn reply(&self, to: &MessageId, text: &str) -> Result<(), TimelineError> {
        let item = self.item(to).await?;
        let event = item.event_id().ok_or(TimelineError::NotSentYet)?.to_owned();
        self.timeline.send_reply(RoomMessageEventContentWithoutRelation::text_plain(text), event).await.map(|_| ()).map_err(sdk)
    }

    /// Adds the user's reaction with `key`, or takes it back if they had given it.
    pub async fn react(&self, message: &MessageId, key: &str) -> Result<(), TimelineError> {
        let item = self.item(message).await?;
        self.timeline.toggle_reaction(&item.identifier(), key).await.map(|_| ()).map_err(sdk)
    }

    /// Replaces the text of one of the user's own messages.
    pub async fn edit(&self, message: &MessageId, text: &str) -> Result<(), TimelineError> {
        let item = self.item(message).await?;
        let content = EditedContent::RoomMessage(RoomMessageEventContentWithoutRelation::text_plain(text));
        self.timeline.edit(&item.identifier(), content).await.map_err(sdk)
    }

    /// Deletes a message for everyone (a redaction). The server refuses other people's messages unless the user
    /// may delete them.
    pub async fn delete(&self, message: &MessageId) -> Result<(), TimelineError> {
        let item = self.item(message).await?;
        self.timeline.redact(&item.identifier(), None).await.map_err(sdk)
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

/// The messages among the timeline's items, oldest first, and the replied-to events that are not loaded. Date
/// dividers, read markers and room state changes are left out: PingCraft shows messages only.
fn messages(items: &Vector<Arc<TimelineItem>>, own: &UserId) -> (Vec<Message>, Vec<OwnedEventId>) {
    let events: Vec<(&TimelineUniqueId, &EventTimelineItem)> = items.iter().filter_map(|i| i.as_event().map(|e| (i.unique_id(), e))).collect();
    let mut names: HashMap<OwnedUserId, String> = HashMap::new();
    let mut ids: HashMap<OwnedEventId, MessageId> = HashMap::new();
    for (id, event) in &events {
        if let TimelineDetails::Ready(Profile { display_name: Some(name), .. }) = event.sender_profile() {
            names.insert(event.sender().to_owned(), name.clone());
        }
        if let Some(event_id) = event.event_id() {
            ids.insert(event_id.to_owned(), MessageId(id.0.clone()));
        }
    }
    let known = Known { names: &names, ids: &ids, own };
    let mut missing = Vec::new();
    let list = events.iter().filter_map(|(id, event)| message(id, event, &known, &mut missing)).collect();
    (list, missing)
}

/// What the loaded messages tell about each other: senders' names, and which event is which message.
struct Known<'a> {
    names: &'a HashMap<OwnedUserId, String>,
    ids: &'a HashMap<OwnedEventId, MessageId>,
    own: &'a UserId,
}

impl Known<'_> {
    fn name(&self, user: &UserId) -> String {
        self.names.get(user).cloned().unwrap_or_else(|| user.to_string())
    }
}

fn message(id: &TimelineUniqueId, event: &EventTimelineItem, known: &Known<'_>, missing: &mut Vec<OwnedEventId>) -> Option<Message> {
    let TimelineItemContent::MsgLike(content) = event.content() else { return None };
    let sender = match event.sender_profile() {
        TimelineDetails::Ready(profile) => profile.display_name.clone().unwrap_or_else(|| event.sender().to_string()),
        _ => event.sender().to_string(),
    };
    let state = match event.send_state() {
        None | Some(EventSendState::Sent { .. }) => SendState::Sent,
        Some(EventSendState::NotSentYet { .. }) => SendState::Sending,
        Some(EventSendState::SendingFailed { error, .. }) => SendState::Failed(error.to_string()),
    };
    let body = kind_body(&content.kind);
    let deleted = body == MessageBody::Deleted;
    let mut m = Message::new(MessageId(id.0.clone()), sender, event.is_own(), Timestamp(u64::from(event.timestamp().0)), body, state);
    m.edited = matches!(&content.kind, MsgLikeKind::Message(message) if message.is_edited());
    m.reactions = reactions(content, known);
    m.reply_to = content.in_reply_to.as_ref().map(|r| quote(r, known, missing));
    let on_server = event.event_id().is_some();
    m.can = Abilities {
        reply: !deleted && on_server && event.can_be_replied_to(),
        react: !deleted && on_server,
        edit: !deleted && event.is_editable(),
        delete: !deleted && event.is_own(),
    };
    Some(m)
}

fn kind_body(kind: &MsgLikeKind) -> MessageBody {
    match kind {
        MsgLikeKind::Message(message) => body_of(message.msgtype()),
        MsgLikeKind::UnableToDecrypt(_) => MessageBody::WaitingForKey,
        MsgLikeKind::Redacted => MessageBody::Deleted,
        MsgLikeKind::Sticker(_) => MessageBody::Unsupported("sticker".into()),
        MsgLikeKind::Poll(_) => MessageBody::Unsupported("poll".into()),
        MsgLikeKind::LiveLocation(_) => MessageBody::Unsupported("location".into()),
        MsgLikeKind::Other(_) => MessageBody::Unsupported("message".into()),
    }
}

/// The reactions to a message, each with who gave it, in the order the SDK keeps (first given first).
fn reactions(content: &MsgLikeContent, known: &Known<'_>) -> Vec<Reaction> {
    content
        .reactions
        .iter()
        .filter(|(_, by)| !by.is_empty())
        .map(|(key, by)| Reaction { key: key.clone(), by: by.keys().map(|u| known.name(u)).collect(), mine: by.contains_key(known.own) })
        .collect()
}

/// The quoted original of a reply. One that is not loaded is noted in `missing`, to be fetched.
fn quote(reply: &InReplyToDetails, known: &Known<'_>, missing: &mut Vec<OwnedEventId>) -> Quote {
    let message = known.ids.get(&reply.event_id).cloned();
    match &reply.event {
        TimelineDetails::Ready(original) => {
            let body = match &original.content {
                TimelineItemContent::MsgLike(c) => kind_body(&c.kind),
                _ => MessageBody::Unsupported("message".into()),
            };
            let sender = match &original.sender_profile {
                TimelineDetails::Ready(Profile { display_name: Some(name), .. }) => name.clone(),
                _ => known.name(&original.sender),
            };
            Quote { message, sender, body }
        }
        TimelineDetails::Unavailable => {
            missing.push(reply.event_id.clone());
            Quote { message, sender: String::new(), body: MessageBody::Loading }
        }
        TimelineDetails::Pending => Quote { message, sender: String::new(), body: MessageBody::Loading },
        TimelineDetails::Error(_) => Quote { message, sender: String::new(), body: MessageBody::Unsupported("message".into()) },
    }
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

//! The inbox: `matrix-sdk-ui`'s room list, mapped onto [`pingcraft_core::Chat`] and sent as [`InboxChange`]s.
//!
//! Which network a chat is on comes from the bridge-info state a bridge puts in its rooms (`m.bridge`, or the
//! older `uk.half-shot.bridge`): `content.protocol.id` of an event with a non-empty state key. Anyone who can send
//! state in a room can send such an event, so one counts only when its sender is the bridge bot it names
//! (`content.bridgebot`) and that bot is on the user's own server, where the admin runs the bridges. A room without
//! one is a Matrix chat. [`crate::bridge_info`] gets that state into the local store and says which rooms changed.
//!
//! A chat's archive, its sound and colour, and its mute's end are PingCraft's own room account data
//! ([`crate::chat_settings`]); whether it is muted is a push rule (`m.push_rules` global account data). The room list
//! notices a change to neither, so the inbox also watches the sync's room updates and the push rules, and sends such a
//! chat again, whether the change was made here or on another device ([`mute_wakes`] says when for a mute).

use std::collections::{HashMap, HashSet};
use std::pin::pin;
use std::sync::Arc;

use futures_util::{Stream, StreamExt, stream};
use matrix_sdk::deserialized_responses::{RawAnySyncOrStrippedState, TimelineEventKind};
use matrix_sdk::event_handler::EventHandlerDropGuard;
use matrix_sdk::executor::{AbortOnDrop, spawn};
use matrix_sdk::latest_events::LatestEventValue;
use matrix_sdk::ruma::events::push_rules::PushRulesEvent;
use matrix_sdk::ruma::events::{AnyRoomAccountDataEvent, StateEventType};
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId, UserId};
use matrix_sdk::sync::RoomUpdates;
use matrix_sdk::{Client, Room};
use matrix_sdk_ui::RoomListService;
use matrix_sdk_ui::eyeball_im::{Vector, VectorDiff};
use matrix_sdk_ui::room_list_service::filters::{new_filter_all, new_filter_joined, new_filter_not, new_filter_space};
use matrix_sdk_ui::room_list_service::{RoomList, RoomListItem};
use pingcraft_core::{Chat, ChatId, InboxChange, MessageBody, MessagePreview, Mute, Network, Timestamp};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::bridge_info::BRIDGE_STATE_TYPES;
use crate::connection::SyncError;

/// How many chats the room list hands over. Far more than anyone has; the limit only bounds memory if a server
/// sends nonsense.
const MAX_CHATS: usize = 50_000;

/// The inbox's changes, in order. Poll it with [`InboxFeed::try_next`] from the interface, or await
/// [`InboxFeed::next`]. Dropping it stops the work behind it.
pub struct InboxFeed {
    changes: UnboundedReceiver<InboxChange>,
    _task: AbortOnDrop<()>,
}

impl InboxFeed {
    pub(crate) async fn start(client: Client, rooms: Arc<RoomListService>, bridges: broadcast::Receiver<OwnedRoomId>) -> Result<Self, SyncError> {
        // Before the room list, so no change falls between the chats first sent and the watch.
        let settings = client.subscribe_to_all_room_updates();
        let push_rules = watch_push_rules(&client);
        let list = rooms.all_rooms().await.map_err(|e| SyncError(e.to_string()))?;
        let (tx, changes) = unbounded_channel();
        let task = spawn(run(client, list, bridges, settings, push_rules, tx));
        Ok(Self { changes, _task: AbortOnDrop::new(task) })
    }

    /// The next change, if one is waiting. Never blocks.
    pub fn try_next(&mut self) -> Option<InboxChange> {
        self.changes.try_recv().ok()
    }

    /// Waits for the next change. `None` when the feed has ended (the sync stopped for good).
    pub async fn next(&mut self) -> Option<InboxChange> {
        self.changes.recv().await
    }
}

/// What wakes the inbox task.
enum Wake {
    /// The room list changed.
    Rooms(Vec<VectorDiff<RoomListItem>>),
    /// A room's bridge info may have changed; `None` means some updates were missed, so look at every room.
    Bridge(Option<OwnedRoomId>),
    /// The sync brought PingCraft's own room account data for these rooms; `None` means some sync updates were
    /// missed, so send every room again.
    Settings(Option<Vec<(OwnedRoomId, Brought)>>),
    /// The push rules changed, so a chat may have been muted or unmuted.
    PushRules,
}

/// Which of PingCraft's own room account data the sync brought for a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Brought {
    /// Its mute's end only: the chat is sent again only when [`mute_wakes`] says so.
    MuteEnd,
    /// Its archive, or its sound and colour ([`WAKING_SETTINGS`]): the chat is always sent again.
    Settings,
}

/// PingCraft's room account data that always sends a chat again when the sync brings it.
const WAKING_SETTINGS: &[&str] = &[crate::chat_settings::ARCHIVED, crate::chat_settings::ALERTS];

/// The joined rooms in a sync's room updates that bring some of PingCraft's own room account data, and what.
fn rooms_with_own_settings(updates: &RoomUpdates) -> Vec<(OwnedRoomId, Brought)> {
    let kind = |raw: &Raw<AnyRoomAccountDataEvent>| match raw.get_field::<String>("type").ok().flatten() {
        Some(kind) if WAKING_SETTINGS.contains(&kind.as_str()) => Some(Brought::Settings),
        Some(kind) if kind == crate::chat_settings::MUTE_UNTIL => Some(Brought::MuteEnd),
        _ => None,
    };
    updates.joined.iter().filter_map(|(id, update)| update.account_data.iter().filter_map(kind).max().map(|b| (id.clone(), b))).collect()
}

/// Whether a chat is sent again for a change to its mute, given whether it was last sent as muted (`None`: never sent),
/// its mute now, and whether its mute's end just arrived (`false`: the push rules changed).
///
/// Whether a chat is muted is read from its push rule, which the SDK reads again for every chat before the push
/// rules' change wakes the inbox; a chat is sent again when that differs from what was last sent. A mute's end is
/// saved before the rule when muting and before the rule is taken away when unmuting
/// ([`crate::Connection::mute`]), so it can arrive while the rule still says the opposite. It sends the chat only while
/// the rule says muted with an end, so neither a mute nor an unmute in progress shows the wrong state: a mute does
/// not show as off before its rule arrives (the reason a mute's end did not wake the inbox at all before), and an
/// unmute does not show as "until turned back on" (its end is cleared to 0 first). It still sends a chat whose rule
/// arrived first (a sync that brings global account data before a room's), so the end time shows when it comes.
fn mute_wakes(last_sent: Option<bool>, now: Option<Mute>, end_arrived: bool) -> bool {
    last_sent != Some(now.is_some()) && !end_arrived || end_arrived && matches!(now, Some(Mute::Until(_)))
}

/// Wakes once for each change of the push rules the sync brings. The SDK has already read every chat's mute from the
/// new rules when its event handlers run. The guard keeps the handler; dropping it stops the wakes.
fn watch_push_rules(client: &Client) -> (UnboundedReceiver<()>, EventHandlerDropGuard) {
    let (tx, rx) = unbounded_channel();
    let handle = client.add_event_handler(move |_: PushRulesEvent| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(());
        }
    });
    (rx, client.event_handler_drop_guard(handle))
}

fn push_rule_updates(rx: UnboundedReceiver<()>) -> impl Stream<Item = Wake> {
    stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|()| (Wake::PushRules, rx)) })
}

/// Wakes for the rooms whose PingCraft settings the sync brought. Updates without any are skipped.
fn settings_updates(rx: broadcast::Receiver<RoomUpdates>) -> impl Stream<Item = Wake> {
    stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(updates) => {
                    let rooms = rooms_with_own_settings(&updates);
                    if !rooms.is_empty() {
                        return Some((Wake::Settings(Some(rooms)), rx));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => return Some((Wake::Settings(None), rx)),
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
}

fn bridge_updates(rx: broadcast::Receiver<OwnedRoomId>) -> impl Stream<Item = Wake> {
    stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(room) => Some((Wake::Bridge(Some(room)), rx)),
            Err(broadcast::error::RecvError::Lagged(_)) => Some((Wake::Bridge(None), rx)),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    })
}

/// The inbox task's view: the rooms in the list, and what was last sent for each.
struct State {
    rooms: Vector<RoomListItem>,
    sent_network: HashMap<OwnedRoomId, Network>,
    /// Whether each chat was last sent as muted.
    sent_muted: HashMap<OwnedRoomId, bool>,
    listening: HashSet<OwnedRoomId>,
    own: Option<OwnedUserId>,
}

impl State {
    async fn chat(&mut self, room: &Room) -> Chat {
        let chat = chat_from_room(room, self.own.as_deref()).await;
        self.sent_network.insert(room.room_id().to_owned(), chat.network.clone());
        self.sent_muted.insert(room.room_id().to_owned(), chat.muted.is_some());
        chat
    }
}

async fn run(
    client: Client,
    list: RoomList,
    bridges: broadcast::Receiver<OwnedRoomId>,
    settings: broadcast::Receiver<RoomUpdates>,
    (push_rules, _push_rules_guard): (UnboundedReceiver<()>, EventHandlerDropGuard),
    tx: UnboundedSender<InboxChange>,
) {
    let (entries, controller) = list.entries_with_dynamic_adapters(MAX_CHATS);
    // Joined rooms that are not spaces. Invites come with the screen that accepts them.
    controller.set_filter(Box::new(new_filter_all(vec![Box::new(new_filter_joined()), Box::new(new_filter_not(Box::new(new_filter_space())))])));
    let mut wakes = pin!(stream::select(
        stream::select(entries.map(Wake::Rooms), bridge_updates(bridges)),
        stream::select(settings_updates(settings), push_rule_updates(push_rules))
    ));
    let latest = client.latest_events().await;
    let mut state = State {
        rooms: Vector::new(),
        sent_network: HashMap::new(),
        sent_muted: HashMap::new(),
        listening: HashSet::new(),
        own: client.user_id().map(ToOwned::to_owned),
    };

    while let Some(wake) = wakes.next().await {
        let changes = match wake {
            Wake::Rooms(diffs) => {
                let changes = room_list_changed(&mut state, diffs).await;
                // The latest event of a room is only worked out for rooms someone listens to.
                let new: Vec<OwnedRoomId> = state.rooms.iter().map(|r| r.room_id().to_owned()).filter(|id| !state.listening.contains(id)).collect();
                for id in new {
                    if latest.listen_to_room(&id).await.is_ok() {
                        state.listening.insert(id);
                    }
                }
                changes
            }
            Wake::Bridge(room) => bridge_changed(&mut state, room).await,
            Wake::Settings(rooms) => settings_changed(&mut state, rooms.as_deref()).await,
            Wake::PushRules => push_rules_changed(&mut state).await,
        };
        for change in changes {
            if tx.send(change).is_err() {
                // Nobody is listening any more.
                return;
            }
        }
    }
}

async fn room_list_changed(state: &mut State, diffs: Vec<VectorDiff<RoomListItem>>) -> Vec<InboxChange> {
    let before: HashSet<OwnedRoomId> = state.rooms.iter().map(|r| r.room_id().to_owned()).collect();
    let mut reset = false;
    let mut touched: Vec<Room> = Vec::new();
    for diff in diffs {
        match &diff {
            VectorDiff::Reset { .. } | VectorDiff::Clear => reset = true,
            VectorDiff::Append { values } => touched.extend(values.iter().map(|r| Room::clone(r))),
            VectorDiff::PushFront { value } | VectorDiff::PushBack { value } | VectorDiff::Insert { value, .. } | VectorDiff::Set { value, .. } => {
                touched.push(Room::clone(value))
            }
            VectorDiff::PopFront | VectorDiff::PopBack | VectorDiff::Remove { .. } | VectorDiff::Truncate { .. } => {}
        }
        diff.apply(&mut state.rooms);
    }
    let after: HashSet<OwnedRoomId> = state.rooms.iter().map(|r| r.room_id().to_owned()).collect();
    for gone in before.difference(&after) {
        state.sent_network.remove(gone);
        state.sent_muted.remove(gone);
        state.listening.remove(gone);
    }

    if reset {
        let rooms: Vec<Room> = state.rooms.iter().map(|r| Room::clone(r)).collect();
        let mut chats = Vec::with_capacity(rooms.len());
        for room in &rooms {
            chats.push(state.chat(room).await);
        }
        return vec![InboxChange::Reset(chats)];
    }
    let mut changes: Vec<InboxChange> = before.difference(&after).map(|id| InboxChange::Remove(ChatId(id.to_string()))).collect();
    let mut seen = HashSet::new();
    // A batch can touch a room more than once; its state now is what counts, so each is sent once.
    for room in touched.iter().rev() {
        if after.contains(room.room_id()) && seen.insert(room.room_id().to_owned()) {
            changes.push(InboxChange::Upsert(Box::new(state.chat(room).await)));
        }
    }
    changes
}

/// Sends a chat again when its network is not what was last sent. Most updates on the bridge connection are new
/// messages, which the room list reports already.
async fn bridge_changed(state: &mut State, room: Option<OwnedRoomId>) -> Vec<InboxChange> {
    let candidates: Vec<Room> = state.rooms.iter().filter(|r| room.as_ref().is_none_or(|id| r.room_id() == id)).map(|r| Room::clone(r)).collect();
    let mut changes = Vec::new();
    for room in candidates {
        let network = network_of(&room, state.own.as_deref()).await;
        if state.sent_network.get(room.room_id()) != Some(&network) {
            changes.push(InboxChange::Upsert(Box::new(state.chat(&room).await)));
        }
    }
    changes
}

/// Sends the chats again whose PingCraft settings the sync brought (`None`: every chat); one whose mute's end alone
/// arrived only as [`mute_wakes`] says. Only rooms in the list are sent: a space, or a room the user left, must not
/// turn up in the inbox.
async fn settings_changed(state: &mut State, rooms: Option<&[(OwnedRoomId, Brought)]>) -> Vec<InboxChange> {
    let candidates: Vec<(Room, Brought)> = state
        .rooms
        .iter()
        .filter_map(|r| match rooms {
            None => Some((Room::clone(r), Brought::Settings)),
            Some(ids) => ids.iter().find(|(id, _)| r.room_id() == id).map(|(_, brought)| (Room::clone(r), *brought)),
        })
        .collect();
    let mut changes = Vec::with_capacity(candidates.len());
    for (room, brought) in candidates {
        let chat = chat_from_room(&room, state.own.as_deref()).await;
        if brought == Brought::MuteEnd && !mute_wakes(state.sent_muted.get(room.room_id()).copied(), chat.muted, true) {
            continue;
        }
        changes.push(InboxChange::Upsert(Box::new(state.chat(&room).await)));
    }
    changes
}

/// Sends the chats again whose push rule now says otherwise than what was last sent: muted or unmuted, here or on
/// another device.
async fn push_rules_changed(state: &mut State) -> Vec<InboxChange> {
    let changed: Vec<Room> = state
        .rooms
        .iter()
        .filter(|r| {
            let now = crate::chat_settings::is_muted(r).then_some(Mute::UntilUnmuted);
            mute_wakes(state.sent_muted.get(r.room_id()).copied(), now, false)
        })
        .map(|r| Room::clone(r))
        .collect();
    let mut changes = Vec::with_capacity(changed.len());
    for room in changed {
        changes.push(InboxChange::Upsert(Box::new(state.chat(&room).await)));
    }
    changes
}

/// One room as the inbox shows it.
pub(crate) async fn chat_from_room(room: &Room, own: Option<&UserId>) -> Chat {
    let name = match room.cached_display_name() {
        Some(name) => name.to_string(),
        None => room.display_name().await.map(|n| n.to_string()).unwrap_or_else(|_| room.room_id().to_string()),
    };
    let (draft, draft_for) = crate::chat_settings::draft(room).await;
    Chat {
        id: ChatId(room.room_id().to_string()),
        account: None,
        network: network_of(room, own).await,
        name,
        last_message: preview(room, own).await,
        unread: u32::try_from(room.num_unread_messages()).unwrap_or(u32::MAX),
        encrypted: room.encryption_state().is_encrypted(),
        pinned: room.is_favourite(),
        muted: match crate::chat_settings::is_muted(room) {
            false => None,
            true => Some(crate::chat_settings::mute_until(room).await.map_or(Mute::UntilUnmuted, Mute::Until)),
        },
        marked_unread: room.is_marked_unread(),
        draft,
        draft_for,
        archived: crate::chat_settings::archived(room).await,
        alerts: crate::chat_settings::alerts(room).await,
    }
}

/// The network a room's bridge says it is on, from the bridge-info state in the local store. `own` is the signed-in
/// user; without one, no bridge is trusted.
pub(crate) async fn network_of(room: &Room, own: Option<&UserId>) -> Network {
    let Some(own) = own else { return Network::matrix() };
    let mut events = Vec::new();
    for kind in BRIDGE_STATE_TYPES {
        let Ok(found) = room.get_state_events(StateEventType::from(*kind)).await else { continue };
        let mut values: Vec<Value> = found
            .iter()
            .filter_map(|raw| {
                let json = match raw {
                    RawAnySyncOrStrippedState::Sync(e) => e.json().get(),
                    RawAnySyncOrStrippedState::Stripped(e) => e.json().get(),
                };
                serde_json::from_str(json).ok()
            })
            .collect();
        values.sort_by(|a, b| state_key(a).cmp(state_key(b)));
        events.extend(values);
    }
    network_from_bridge_events(&events, own.server_name().as_str())
}

fn state_key(event: &Value) -> &str {
    event.get("state_key").and_then(Value::as_str).unwrap_or_default()
}

/// The network named by the first trusted bridge-info event that has a non-empty state key and a protocol ID; Matrix
/// when there is none. Events are taken as hostile input: anything missing or of the wrong type is skipped, and so
/// is an event that was not sent by a bridge bot on `own_server` ([`sent_by_a_local_bridge`]).
pub(crate) fn network_from_bridge_events(events: &[Value], own_server: &str) -> Network {
    events
        .iter()
        .filter(|e| !state_key(e).is_empty())
        .filter(|e| sent_by_a_local_bridge(e, own_server))
        .filter_map(|e| e.pointer("/content/protocol/id").and_then(Value::as_str))
        .map(Network::from_id)
        .find(|n| !n.is_matrix())
        .unwrap_or_else(Network::matrix)
}

/// Whether a bridge-info event was sent by the bridge bot it names, and that bot is on `own_server`. mautrix bridges
/// always send it as their bot. Without this check anyone in a room (or a bridge on another server, once federation
/// is on) could label a chat as any network: a Matrix chat could pass itself off as a family member's WhatsApp chat.
fn sent_by_a_local_bridge(event: &Value, own_server: &str) -> bool {
    let sender = event.get("sender").and_then(Value::as_str);
    let bot = event.pointer("/content/bridgebot").and_then(Value::as_str);
    match (sender, bot) {
        (Some(sender), Some(bot)) if sender == bot => UserId::parse(bot).is_ok_and(|bot| bot.server_name().as_str() == own_server),
        _ => false,
    }
}

/// What a message's content shows as. `content` is an `m.room.message` content object.
pub(crate) fn body_from_message_content(content: &Value) -> MessageBody {
    let msgtype = content.get("msgtype").and_then(Value::as_str).unwrap_or_default();
    let text = content.get("body").and_then(Value::as_str);
    match (msgtype, text) {
        ("m.text" | "m.notice" | "m.emote", Some(text)) => MessageBody::Text(text.to_owned()),
        ("m.image", _) => MessageBody::Unsupported("image".into()),
        ("m.file", _) => MessageBody::Unsupported("file".into()),
        ("m.audio", _) => MessageBody::Unsupported("audio".into()),
        ("m.video", _) => MessageBody::Unsupported("video".into()),
        ("m.location", _) => MessageBody::Unsupported("location".into()),
        _ => MessageBody::Unsupported("message".into()),
    }
}

/// What an event shows as in the inbox, from its type and content.
pub(crate) fn body_from_event(event_type: &str, content: &Value) -> MessageBody {
    match event_type {
        "m.room.message" => body_from_message_content(content),
        "m.room.encrypted" => MessageBody::WaitingForKey,
        "m.sticker" => MessageBody::Unsupported("sticker".into()),
        "m.poll.start" | "org.matrix.msc3381.poll.start" => MessageBody::Unsupported("poll".into()),
        _ => MessageBody::Unsupported("message".into()),
    }
}

async fn sender_name(room: &Room, user: &UserId) -> String {
    match room.get_member_no_sync(user).await {
        Ok(Some(member)) => member.display_name().map(str::to_owned).unwrap_or_else(|| user.to_string()),
        _ => user.to_string(),
    }
}

async fn preview(room: &Room, own: Option<&UserId>) -> Option<MessagePreview> {
    let latest = room.latest_event();
    let at = Timestamp(latest.timestamp().map(|t| u64::from(t.0)).unwrap_or_default());
    let (sender, body): (Option<OwnedUserId>, MessageBody) = match latest {
        LatestEventValue::None | LatestEventValue::RemoteInvite { .. } => return None,
        LatestEventValue::Remote(event) => {
            let body = match &event.kind {
                TimelineEventKind::UnableToDecrypt { .. } => MessageBody::WaitingForKey,
                _ => {
                    let json: Value = serde_json::from_str(event.raw().json().get()).unwrap_or_default();
                    let kind = json.get("type").and_then(Value::as_str).unwrap_or_default();
                    body_from_event(kind, json.get("content").unwrap_or(&Value::Null))
                }
            };
            (event.sender(), body)
        }
        LatestEventValue::LocalIsSending(value) | LatestEventValue::LocalCannotBeSent(value) | LatestEventValue::LocalHasBeenSent { value, .. } => {
            let (raw, kind) = value.content.raw();
            let content: Value = serde_json::from_str(raw.json().get()).unwrap_or_default();
            (own.map(ToOwned::to_owned), body_from_event(kind, &content))
        }
    };
    let sender_id = sender?;
    Some(MessagePreview { own: own == Some(sender_id.as_ref()), sender: sender_name(room, &sender_id).await, body, at })
}

#[cfg(test)]
mod tests {
    use super::{body_from_event, body_from_message_content, network_from_bridge_events, rooms_with_own_settings};
    use matrix_sdk::ruma::serde::Raw;
    use matrix_sdk::ruma::{OwnedRoomId, owned_room_id};
    use matrix_sdk::sync::{JoinedRoomUpdate, LeftRoomUpdate, RoomUpdates};
    use pingcraft_core::{MessageBody, Network};
    use serde_json::{Value, json};

    const SERVER: &str = "example.org";

    /// A bridge-info event as a bridge on this server sends it: from its bot, naming its bot.
    fn bridge_event(state_key: &str, protocol: Value) -> Value {
        json!({"type": "m.bridge", "state_key": state_key, "sender": "@signalbot:example.org", "content": {"bridgebot": "@signalbot:example.org", "protocol": {"id": protocol}}})
    }

    #[test]
    fn the_bridge_names_the_network() {
        let e = json!({"type": "m.bridge", "state_key": "net.maunium.telegram://telegram/1", "sender": "@telegrambot:example.org", "content": {"bridgebot": "@telegrambot:example.org", "protocol": {"id": "Telegram"}}});
        assert_eq!(network_from_bridge_events(&[e], SERVER), Network::from_id("telegram"));
    }

    #[test]
    fn an_empty_state_key_or_a_missing_protocol_means_matrix() {
        let empty_key = bridge_event("", json!("telegram"));
        let no_protocol = json!({"state_key": "x", "sender": "@signalbot:example.org", "content": {"bridgebot": "@signalbot:example.org"}});
        let wrong_type = bridge_event("x", json!(7));
        let blank = bridge_event("x", json!("  "));
        assert!(network_from_bridge_events(&[empty_key, no_protocol, wrong_type, blank], SERVER).is_matrix());
        assert!(network_from_bridge_events(&[], SERVER).is_matrix());
    }

    #[test]
    fn the_first_usable_event_wins() {
        let skipped = bridge_event("", json!("discord"));
        let signal = bridge_event("a", json!("signal"));
        let whatsapp = bridge_event("b", json!("whatsapp"));
        assert_eq!(network_from_bridge_events(&[skipped, signal, whatsapp], SERVER), Network::from_id("signal"));
    }

    #[test]
    fn only_a_bridge_bot_on_this_server_names_the_network() {
        // Someone in the room names the real bot, and their event sorts ahead of the bridge's own.
        let spoof = json!({"state_key": "a", "sender": "@mallory:example.org", "content": {"bridgebot": "@signalbot:example.org", "protocol": {"id": "whatsapp"}}});
        let genuine = bridge_event("b", json!("signal"));
        assert_eq!(network_from_bridge_events(&[spoof.clone(), genuine], SERVER), Network::from_id("signal"));
        assert!(network_from_bridge_events(&[spoof], SERVER).is_matrix(), "the sender is not the bot it names");

        // A bot naming itself, but on another server: not a bridge this server's admin runs.
        let remote = json!({"state_key": "a", "sender": "@whatsappbot:evil.example", "content": {"bridgebot": "@whatsappbot:evil.example", "protocol": {"id": "whatsapp"}}});
        let lookalike = json!({"state_key": "a", "sender": "@bot:example.org.evil", "content": {"bridgebot": "@bot:example.org.evil", "protocol": {"id": "whatsapp"}}});
        assert!(network_from_bridge_events(&[remote, lookalike], SERVER).is_matrix());

        // A missing or malformed sender or bot.
        let no_sender = json!({"state_key": "a", "content": {"bridgebot": "@signalbot:example.org", "protocol": {"id": "signal"}}});
        let no_bot = json!({"state_key": "a", "sender": "@signalbot:example.org", "content": {"protocol": {"id": "signal"}}});
        let not_an_id = json!({"state_key": "a", "sender": "signalbot", "content": {"bridgebot": "signalbot", "protocol": {"id": "signal"}}});
        let wrong_types = json!({"state_key": "a", "sender": 1, "content": {"bridgebot": ["@signalbot:example.org"], "protocol": {"id": "signal"}}});
        assert!(network_from_bridge_events(&[no_sender, no_bot, not_an_id, wrong_types], SERVER).is_matrix());
    }

    /// Room account data of these types, as a sync brings it.
    fn account_data(types: &[&str]) -> Vec<Raw<matrix_sdk::ruma::events::AnyRoomAccountDataEvent>> {
        types.iter().map(|t| Raw::new(&json!({"type": t, "content": {"at": 1}})).unwrap().cast_unchecked()).collect()
    }

    fn joined(types: &[&str]) -> JoinedRoomUpdate {
        JoinedRoomUpdate { account_data: account_data(types), ..Default::default() }
    }

    /// Archiving, a chat's sound and colour and a mute's end are PingCraft's own room account data, which changes
    /// nothing the room list watches: the inbox must hear of them from the sync itself. A mute's end is told apart
    /// ([`super::mute_wakes`] decides); other apps' account data and rooms the user left wake nothing.
    #[test]
    fn pingcraft_settings_in_joined_rooms_wake_the_inbox() {
        use super::Brought;
        let (archived, alerts, muted, both, tagged, left): (OwnedRoomId, OwnedRoomId, OwnedRoomId, OwnedRoomId, OwnedRoomId, OwnedRoomId) = (
            owned_room_id!("!archived:example.org"),
            owned_room_id!("!alerts:example.org"),
            owned_room_id!("!muted:example.org"),
            owned_room_id!("!both:example.org"),
            owned_room_id!("!tagged:example.org"),
            owned_room_id!("!left:example.org"),
        );
        let mut updates = RoomUpdates::default();
        updates.joined.insert(archived.clone(), joined(&[crate::chat_settings::ARCHIVED]));
        updates.joined.insert(alerts.clone(), joined(&["m.tag", crate::chat_settings::ALERTS]));
        updates.joined.insert(muted.clone(), joined(&[crate::chat_settings::MUTE_UNTIL]));
        updates.joined.insert(both.clone(), joined(&[crate::chat_settings::MUTE_UNTIL, crate::chat_settings::ARCHIVED]));
        updates.joined.insert(tagged, joined(&["m.tag", "m.fully_read", "io.github.jonasbove.pingcraftish.archived", "com.example.archived"]));
        updates.left.insert(left, LeftRoomUpdate { account_data: account_data(&[crate::chat_settings::ARCHIVED]), ..Default::default() });
        let mut woken = rooms_with_own_settings(&updates);
        woken.sort();
        let mut expected = vec![(archived, Brought::Settings), (alerts, Brought::Settings), (muted, Brought::MuteEnd), (both, Brought::Settings)];
        expected.sort();
        assert_eq!(woken, expected);
        assert!(rooms_with_own_settings(&RoomUpdates::default()).is_empty());
    }

    /// A mute made or ended here or on another device shows in the inbox once its push rule has synced, and the end
    /// and the rule never race into the wrong state (the end is saved before the rule both ways).
    #[test]
    fn a_mute_wakes_the_inbox_only_into_a_right_state() {
        use super::mute_wakes;
        use pingcraft_core::{Mute, Timestamp};
        let until = Some(Mute::Until(Timestamp(5)));
        let for_good = Some(Mute::UntilUnmuted);
        // The push rules changed: sent again only when muted or not differs from what was last sent.
        assert!(mute_wakes(Some(false), until, false), "muted on another device");
        assert!(mute_wakes(Some(false), for_good, false), "muted for good on another device");
        assert!(mute_wakes(Some(true), None, false), "turned back on on another device");
        assert!(!mute_wakes(Some(true), until, false), "still muted: another chat's rule changed");
        assert!(!mute_wakes(Some(false), None, false), "still not muted");
        assert!(mute_wakes(None, None, false), "never sent");
        // A mute's end arrived.
        assert!(!mute_wakes(Some(false), None, true), "muting: the end came before the rule, which would show it as not muted");
        assert!(!mute_wakes(Some(true), for_good, true), "unmuting: the end was cleared before the rule went, which would show it muted for good");
        assert!(mute_wakes(Some(true), until, true), "the rule came first (or with it): the end shows when it arrives");
        assert!(mute_wakes(Some(false), until, true), "the end and the rule arrived together");
    }

    #[test]
    fn message_bodies_map_by_msgtype() {
        assert_eq!(body_from_message_content(&json!({"msgtype": "m.text", "body": "hi"})), MessageBody::Text("hi".into()));
        assert_eq!(body_from_message_content(&json!({"msgtype": "m.notice", "body": "n"})), MessageBody::Text("n".into()));
        assert_eq!(body_from_message_content(&json!({"msgtype": "m.image", "body": "x.png"})), MessageBody::Unsupported("image".into()));
        assert_eq!(body_from_message_content(&json!({"msgtype": "m.text"})), MessageBody::Unsupported("message".into()));
        assert_eq!(body_from_message_content(&json!("nonsense")), MessageBody::Unsupported("message".into()));
        assert_eq!(body_from_event("m.room.encrypted", &json!({})), MessageBody::WaitingForKey);
        assert_eq!(body_from_event("m.sticker", &json!({})), MessageBody::Unsupported("sticker".into()));
    }
}

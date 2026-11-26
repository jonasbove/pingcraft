//! Keeping up with the server: `matrix-sdk-ui`'s `SyncService`, over simplified sliding sync (MSC4186).
//!
//! A [`Connection`] owns the sync. The inbox ([`crate::inbox`]) and the open chats ([`crate::timeline`]) are fed
//! from it, as `pingcraft-core` types; nothing from the SDK leaves this crate.

use matrix_sdk::Client;
use matrix_sdk_ui::sync_service::{State, SyncService};
use pingcraft_core::ConnectionState;

use crate::bridge_info::BridgeInfo;
use crate::inbox::InboxFeed;

/// Why the sync could not start, in the SDK's words, for the log.
#[derive(Debug)]
pub struct SyncError(pub String);

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "could not sync: {}", self.0)
    }
}

impl std::error::Error for SyncError {}

/// A signed-in client that is syncing. Call [`Connection::stop`] before signing out.
pub struct Connection {
    client: Client,
    sync: SyncService,
    bridges: BridgeInfo,
}

impl Connection {
    /// Starts syncing, over the homeserver's simplified sliding sync (tuwunel serves it).
    pub async fn start(client: Client) -> Result<Self, SyncError> {
        let sync = SyncService::builder(client.clone()).with_offline_mode().build().await.map_err(|e| SyncError(e.to_string()))?;
        let bridges = BridgeInfo::new(&client).await.map_err(|e| SyncError(e.to_string()))?;
        sync.start().await;
        bridges.start();
        Ok(Self { client, sync, bridges })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn sync_service(&self) -> &SyncService {
        &self.sync
    }

    /// Where the sync is now. Cheap; the interface can ask every frame.
    pub fn state(&self) -> ConnectionState {
        match self.sync.state().get() {
            State::Idle => ConnectionState::Connecting,
            State::Running => ConnectionState::Online,
            State::Offline => ConnectionState::Offline,
            State::Terminated => ConnectionState::Stopped,
            State::Error(e) => ConnectionState::Error(e.to_string()),
        }
    }

    /// The inbox, as changes to apply to a [`pingcraft_core::Inbox`].
    pub async fn inbox(&self) -> Result<InboxFeed, SyncError> {
        InboxFeed::start(self.client.clone(), self.sync.room_list_service(), self.bridges.subscribe()).await
    }

    /// Restarts the sync after an error or after being offline. Harmless when it is running.
    pub async fn resume(&self) {
        self.sync.start().await;
        self.bridges.start();
    }

    /// Turns on end-to-end encryption in a chat: sends its `m.room.encryption` state, which the server keeps for
    /// good. Fails when the user may not change the chat's settings.
    pub async fn enable_encryption(&self, chat: &pingcraft_core::ChatId) -> Result<(), String> {
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(chat.0.as_str()).map_err(|_| "no such chat".to_owned())?;
        let room = self.client.get_room(&room_id).ok_or_else(|| "no such chat".to_owned())?;
        if room.latest_encryption_state().await.map(|s| s.is_encrypted()).unwrap_or(false) {
            return Ok(());
        }
        room.enable_encryption().await.map_err(|e| match e.as_client_api_error().map(|e| e.status_code.as_u16()) {
            Some(403) => "you don't have permission to change this chat's settings, so encryption can't be turned on".to_owned(),
            _ => format!("encryption could not be turned on: {e}"),
        })
    }

    /// Sends a text message to any chat in the inbox, open or not, through its send queue (so it is sent again
    /// after a lost connection).
    pub async fn send_text_to(&self, chat: &pingcraft_core::ChatId, text: &str) -> Result<(), String> {
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(chat.0.as_str()).map_err(|_| "no such chat".to_owned())?;
        let room = self.client.get_room(&room_id).ok_or_else(|| "no such chat".to_owned())?;
        let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(text);
        room.send_queue().send(content.into()).await.map(|_| ()).map_err(|e| format!("the message could not be sent: {e}"))
    }

    /// Ends the current sync connection and starts a new one, so the server is asked again now rather than when the
    /// long poll returns. The bridges' room-state watch keeps running.
    pub async fn refresh(&self) {
        self.sync.stop().await;
        self.sync.start().await;
    }

    /// Stops syncing, for example before signing out.
    pub async fn stop(&self) {
        self.bridges.stop();
        self.sync.stop().await;
    }
}

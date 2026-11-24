//! Keeping each room's bridge-info state (`m.bridge`, `uk.half-shot.bridge`) in the local store, so the inbox can
//! tell which network a chat is on.
//!
//! The room list's sliding sync asks for a fixed set of state that does not include it, so this runs a second,
//! small sliding sync connection with one list over every room that asks only for that state. It is not cached and
//! does not share its position: on each start it fetches the bridge info of every room again, which is small, and
//! then long-polls for changes. When it fails, chats stay labelled as Matrix until it recovers; the inbox keeps
//! working, so its failures never count as the connection failing.
//!
//! This is the one place that knows how bridge info gets here. If a server does not honour the list, the
//! alternative (fetching `/state` per room) replaces this file only.

use std::sync::Mutex;
use std::time::Duration;

use futures_util::StreamExt;
use matrix_sdk::executor::{AbortOnDrop, spawn};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::StateEventType;
use matrix_sdk::sleep::sleep;
use matrix_sdk::{Client, SlidingSync, SlidingSyncList, SlidingSyncMode};
use tokio::sync::broadcast;

/// The bridge-info state event types, the current one first.
pub(crate) const BRIDGE_STATE_TYPES: &[&str] = &["m.bridge", "uk.half-shot.bridge"];

/// The sliding sync connection ID, separate from the room list's.
const CONNECTION_ID: &str = "bridge-info";
const LIST: &str = "bridges";
/// Rooms per request while the list fills.
const BATCH: u32 = 100;
/// How many room updates may wait for a slow inbox before it is told to look at every room again.
const PENDING: usize = 1024;
/// Waits between attempts after an error: doubling up to the last.
const BACKOFF: Duration = Duration::from_secs(2);
const BACKOFF_MAX: Duration = Duration::from_secs(120);

/// The second sliding sync connection, and the rooms it has seen updates for.
pub(crate) struct BridgeInfo {
    sliding_sync: SlidingSync,
    updates: broadcast::Sender<OwnedRoomId>,
    task: Mutex<Option<AbortOnDrop<()>>>,
}

impl BridgeInfo {
    pub(crate) async fn new(client: &Client) -> Result<Self, matrix_sdk::Error> {
        let list = SlidingSyncList::builder(LIST)
            .sync_mode(SlidingSyncMode::new_growing(BATCH))
            // Like the room list's own list: one event, so a response never looks like a gap with nothing in it.
            .timeline_limit(1)
            .required_state(BRIDGE_STATE_TYPES.iter().map(|t| (StateEventType::from(*t), "*".to_owned())).collect());
        let sliding_sync = client.sliding_sync(CONNECTION_ID)?.add_list(list).build().await?;
        let (updates, _) = broadcast::channel(PENDING);
        Ok(Self { sliding_sync, updates, task: Mutex::new(None) })
    }

    /// The rooms whose state may have changed. A lagging receiver should look at every room again.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<OwnedRoomId> {
        self.updates.subscribe()
    }

    /// Starts the sync loop, or does nothing if it is running.
    pub(crate) fn start(&self) {
        let Ok(mut task) = self.task.lock() else { return };
        // The loop only ends when it is stopped.
        if task.is_some() {
            return;
        }
        let sliding_sync = self.sliding_sync.clone();
        let updates = self.updates.clone();
        *task = Some(AbortOnDrop::new(spawn(run(sliding_sync, updates))));
    }

    /// Stops the sync loop, and waits for nothing: the loop is cancelled where it is.
    pub(crate) fn stop(&self) {
        if let Ok(mut task) = self.task.lock() {
            task.take();
        }
        let _ = self.sliding_sync.stop_sync();
    }
}

async fn run(sliding_sync: SlidingSync, updates: broadcast::Sender<OwnedRoomId>) {
    let mut wait = BACKOFF;
    loop {
        {
            let stream = sliding_sync.sync();
            let mut stream = std::pin::pin!(stream);
            while let Some(result) = stream.next().await {
                match result {
                    Ok(summary) => {
                        wait = BACKOFF;
                        for room in summary.rooms {
                            // No receiver is fine: no inbox is open.
                            let _ = updates.send(room);
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        sleep(wait).await;
        wait = (wait * 2).min(BACKOFF_MAX);
    }
}

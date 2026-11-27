//! Whether notifications show the message's text, kept with the account so it follows the user between devices
//! (maintainer, 2026-11-26): PingCraft's own global account data [`NOTIFICATIONS`], `{"previews": false}`. Before
//! then it was kept on each computer; the interface moves a computer's "off" up once to an account that keeps none.

use serde_json::{Value, json};

use crate::account_data::synced;
use crate::connection::Connection;
use crate::emoji_settings::TIMEOUT;

/// PingCraft's global account data for notifications.
pub(crate) const NOTIFICATIONS: &str = crate::namespaced!("notifications");

/// The previews setting in the content, if it holds a true or false one.
fn previews(content: &Value) -> Option<bool> {
    content.get("previews").and_then(Value::as_bool)
}

impl Connection {
    /// The previews setting as synced (no request); `None` when the sync has brought none.
    pub async fn synced_previews(&self) -> Option<bool> {
        synced(&self.client().account(), NOTIFICATIONS).await.as_ref().and_then(previews)
    }

    /// The previews setting the account keeps, asked of the server now; `Ok(None)` when it keeps none.
    pub async fn fetch_previews(&self) -> Result<Option<bool>, String> {
        match tokio::time::timeout(TIMEOUT, self.fetch_global(NOTIFICATIONS)).await {
            Ok(Ok(content)) => Ok(content.as_ref().and_then(previews)),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("the server took too long".to_owned()),
        }
    }

    /// Keeps the previews setting with the account, on top of what the server has (other keys are kept).
    pub async fn save_previews(&self, on: bool) -> Result<(), String> {
        let change = self.merge_global(NOTIFICATIONS, |content| {
            content.insert("previews".to_owned(), json!(on));
        });
        match tokio::time::timeout(TIMEOUT, change).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(format!("whether notifications show message text could not be saved: {e}")),
            Err(_) => Err("whether notifications show message text could not be saved: the server took too long".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{NOTIFICATIONS, previews};

    #[test]
    fn only_a_true_or_false_previews_setting_is_read() {
        assert_eq!(NOTIFICATIONS, "io.github.jonasbove.pingcraft.notifications");
        assert_eq!(previews(&json!({ "previews": false })), Some(false));
        assert_eq!(previews(&json!({ "previews": true, "later": 1 })), Some(true));
        assert_eq!(previews(&json!({ "previews": "no" })), None);
        assert_eq!(previews(&json!({})), None);
        assert_eq!(previews(&json!([false])), None);
    }
}

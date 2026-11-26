//! Global account data, the way PingCraft keeps things with the account: the synced copy, the server's copy asked
//! for now, and a change to some keys made on top of the server's copy, keeping every other key (another app's, or
//! a later PingCraft's) as it is.
//!
//! Each request has a short timeout and no retries: these run on the engine's loop, which must not stall on a
//! network that does not answer. Callers try again later.

use std::time::Duration;

use matrix_sdk::config::RequestConfig;
use matrix_sdk::ruma::api::client::config::{get_global_account_data, set_global_account_data};
use matrix_sdk::ruma::api::error::ErrorKind;
use matrix_sdk::ruma::events::GlobalAccountDataEventType;
use matrix_sdk::ruma::serde::Raw;
use serde_json::{Map, Value};

use crate::connection::Connection;

/// How long one request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// One request's settings: a short timeout and no retries.
pub(crate) fn once() -> RequestConfig {
    RequestConfig::new().timeout(REQUEST_TIMEOUT).disable_retry()
}

/// The synced copy of an event's content, from the store (no request). `None` when the sync has not brought one,
/// which does not mean the account has none.
pub(crate) async fn synced(account: &matrix_sdk::Account, kind: &str) -> Option<Value> {
    account.account_data_raw(GlobalAccountDataEventType::from(kind)).await.ok().flatten().and_then(|raw| raw.deserialize_as_unchecked::<Value>().ok())
}

/// `content` as an object to change: other kinds of content (none, or not an object) start empty.
pub(crate) fn object(content: Option<Value>) -> Map<String, Value> {
    match content {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

impl Connection {
    /// The server's copy of an event's content, asked for now (not the synced one). `None` when the account has
    /// none.
    pub(crate) async fn fetch_global(&self, kind: &str) -> Result<Option<Value>, matrix_sdk::HttpError> {
        let Some(user) = self.client().user_id() else { return Ok(None) };
        let request = get_global_account_data::v3::Request::new(user.to_owned(), GlobalAccountDataEventType::from(kind));
        match self.client().send(request).with_request_config(once()).await {
            Ok(response) => Ok(response.account_data.deserialize_as_unchecked::<Value>().ok()),
            Err(e) if e.client_api_error_kind() == Some(&ErrorKind::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Replaces an event's content on the server.
    pub(crate) async fn put_global(&self, kind: &str, content: &Value) -> Result<(), String> {
        let Some(user) = self.client().user_id() else { return Err("not signed in".to_owned()) };
        let raw = Raw::new(content).map_err(|e| e.to_string())?.cast_unchecked();
        let request = set_global_account_data::v3::Request::new_raw(user.to_owned(), GlobalAccountDataEventType::from(kind), raw);
        self.client().send(request).with_request_config(once()).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Changes some keys of an event's content on top of the server's copy as it is now, keeping the rest, and
    /// returns the content saved.
    pub(crate) async fn merge_global(&self, kind: &str, change: impl FnOnce(&mut Map<String, Value>)) -> Result<Value, String> {
        let mut content = object(self.fetch_global(kind).await.map_err(|e| e.to_string())?);
        change(&mut content);
        let content = Value::Object(content);
        self.put_global(kind, &content).await?;
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::object;

    #[test]
    fn only_an_object_is_built_on() {
        assert_eq!(object(Some(json!({ "a": 1 }))).get("a"), Some(&json!(1)));
        assert!(object(Some(json!([1, 2]))).is_empty());
        assert!(object(Some(json!("text"))).is_empty());
        assert!(object(None).is_empty());
    }
}

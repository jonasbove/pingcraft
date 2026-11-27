//! Each network's notification sound and accent colour, kept with the account so they follow the user between
//! devices: PingCraft's own global account data [`NETWORKS`], `{"telegram": {"sound": false, "accent": "#1c56c4"}}`,
//! one entry per network that has settings. A chat's own settings (`chat_settings::ALERTS`) win over these.

use std::collections::BTreeMap;

use pingcraft_core::{Alerts, Network};
use serde_json::Value;

use crate::account_data::synced;
use crate::chat_settings::{alerts_from, alerts_json};
use crate::connection::Connection;

/// PingCraft's global account data for networks.
pub(crate) const NETWORKS: &str = crate::namespaced!("networks");
/// The longest network ID taken from the server.
const MAX_ID: usize = 64;
/// How many networks are read at most.
const MAX_NETWORKS: usize = 64;

/// Each network's alerts in the content; entries with an unusable ID or no settings are left out.
pub(crate) fn networks(content: &Value) -> BTreeMap<Network, Alerts> {
    let Some(map) = content.as_object() else { return BTreeMap::new() };
    map.iter()
        .filter(|(id, _)| !id.is_empty() && id.len() <= MAX_ID)
        .map(|(id, v)| (Network::from_id(id), alerts_from(v)))
        .filter(|(_, a)| *a != Alerts::default())
        .take(MAX_NETWORKS)
        .collect()
}

impl Connection {
    /// Each network's alerts as synced (no request), with a text that changes when they do (compared, never shown).
    pub async fn network_alerts(&self) -> (BTreeMap<Network, Alerts>, String) {
        let content = synced(&self.client().account(), NETWORKS).await;
        let seen = content.as_ref().map(Value::to_string).unwrap_or_default();
        (content.as_ref().map(networks).unwrap_or_default(), seen)
    }

    /// Sets one network's alerts, on top of what the server has (other networks and keys are kept).
    pub async fn set_network_alerts(&self, network: &Network, alerts: Alerts) -> Result<(), String> {
        let change = self.merge_global(NETWORKS, |content| {
            if alerts == Alerts::default() {
                content.remove(network.id());
            } else {
                content.insert(network.id().to_owned(), alerts_json(alerts));
            }
        });
        match tokio::time::timeout(crate::emoji_settings::TIMEOUT, change).await {
            Ok(result) => result.map(|_| ()).map_err(|e| format!("the network's sound and colour could not be saved: {e}")),
            Err(_) => Err("the network's sound and colour could not be saved: the server took too long".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use pingcraft_core::{Accent, Alerts, Network};
    use serde_json::json;

    use super::{NETWORKS, networks};

    #[test]
    fn each_networks_settings_are_read_and_nonsense_left_out() {
        assert_eq!(NETWORKS, "io.github.jonasbove.pingcraft.networks");
        let read = networks(&json!({
            "telegram": { "sound": false },
            "signal": { "accent": "whatsapp", "x.later": 1 },
            "matrix": { "accent": "#A0B1C2" },
            "discord": {},
            "": { "sound": true },
            "whatsapp": "loud",
        }));
        assert_eq!(read.len(), 3);
        assert_eq!(read[&Network::from_id("telegram")], Alerts { sound: Some(false), accent: None });
        assert_eq!(
            read[&Network::from_id("signal")],
            Alerts { sound: None, accent: Accent::of_network("whatsapp") },
            "saved before any colour could be chosen"
        );
        assert_eq!(read[&Network::matrix()], Alerts { sound: None, accent: Some(Accent([0xa0, 0xb1, 0xc2])) });
        assert!(networks(&json!([])).is_empty());
    }
}

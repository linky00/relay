use relay_daemon::config::RelayData;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RelaytConfig {
    name: String,
    listening: Option<bool>,
    listening_port: Option<u16>,
    initial_ttl: Option<u8>,
    max_forwarding_ttl: Option<u8>,
    #[serde(rename = "paired_relays")]
    trusted_relays: Vec<RelayData>,
}

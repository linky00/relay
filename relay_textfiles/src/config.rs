use relay_daemon::config::RelayData;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RelaytConfig {
    pub name: String,
    #[serde(default)]
    pub listening: bool,
    pub listening_port: Option<u16>,
    pub initial_ttl: Option<u8>,
    pub max_forwarding_ttl: Option<u8>,
    #[serde(rename = "paired_relays")]
    #[serde(default)]
    pub trusted_relays: Vec<RelayData>,
}

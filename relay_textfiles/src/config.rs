use relay_daemon::config::RelayData;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RelaytConfig {
    pub name: String,
    #[serde(default)]
    pub listener: Option<ListeningConfig>,
    pub initial_ttl: Option<u8>,
    pub max_forwarding_ttl: Option<u8>,
    #[serde(rename = "paired_relays")]
    #[serde(default)]
    pub trusted_relays: Vec<RelayData>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ListeningConfig {
    pub port: Option<u16>,
}

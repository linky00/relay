use std::fmt::Display;

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

impl Display for RelaytConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Name: {}", self.name)?;
        if let Some(initial_ttl) = self.initial_ttl {
            writeln!(f, "Initial TTL: {initial_ttl}")?;
        }
        if let Some(max_forwarding_ttl) = self.max_forwarding_ttl {
            writeln!(f, "Max forwarding TTL: {max_forwarding_ttl}")?;
        }
        for relay in &self.trusted_relays {
            writeln!(f, "Paired with:")?;
            if let Some(nickname) = &relay.nickname {
                writeln!(f, "  Nickname: {nickname}")?;
            }
            writeln!(f, "  Key: {}", relay.key)?;
            if let Some(endpoint) = relay.endpoint() {
                writeln!(f, "  Endpoint: {endpoint}")?;
            }
        }
        if let Some(listener) = &self.listener {
            writeln!(f, "Listening!")?;
            if let Some(port) = listener.port {
                writeln!(f, "Port: {port}")?;
            }
        }

        Ok(())
    }
}

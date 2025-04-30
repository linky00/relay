use relay_daemon::config::RelayData;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RelaytConfig {
    name: String,
    is_listener: bool,
    trusted_relays: Vec<RelayData>,
    listening_custom_port: Option<u16>,
    custom_initial_ttl: Option<u8>,
    custom_max_forwarding_ttl: Option<u8>,
}

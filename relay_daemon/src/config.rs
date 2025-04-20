use relay_core::crypto::{PublicKey, SecretKey};

#[derive(Clone)]
pub struct Config {
    pub name: String,
    pub secret_key: SecretKey,
    pub trusted_relays: Vec<RelayData>,
    pub initial_ttl: Option<u8>,
    pub max_forwarding_ttl: Option<u8>,
}

impl Config {
    pub(crate) fn trusted_public_keys(&self) -> Vec<PublicKey> {
        self.trusted_relays.iter().map(|relay| relay.key).collect()
    }
}

#[derive(Clone)]
pub struct RelayData {
    pub key: PublicKey,
    pub host: Option<String>,
    pub nickname: Option<String>,
}

pub trait GetConfig {
    fn get(&self) -> Option<&Config>;
}

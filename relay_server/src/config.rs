use std::collections::HashSet;

pub struct Config {
    name: String,
    trusted_keys: HashSet<String>,
    contacting_hosts: HashSet<String>,
    initial_ttl: Option<u8>,
    max_forwarding_ttl: Option<u8>,
}

pub trait ReadConfig {
    fn read_config(&self) -> &Config;
}

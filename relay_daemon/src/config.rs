use std::collections::HashSet;

pub struct Config {
    pub name: String,
    pub trusted_keys: HashSet<String>,
    pub contacting_hosts: HashSet<String>,
    pub initial_ttl: Option<u8>,
    pub max_forwarding_ttl: Option<u8>,
}

pub trait ReadConfig {
    fn read(&self) -> Option<&Config>;
}

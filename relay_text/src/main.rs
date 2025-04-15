use std::{collections::HashSet, sync::Arc};

use relay_daemon::{
    config::{Config, ReadConfig},
    daemon::RelayDaemon,
};

#[tokio::main]
async fn main() {
    let text_config = TextConfig(Config {
        name: "blah".to_owned(),
        trusted_keys: HashSet::new(),
        contacting_hosts: HashSet::new(),
        initial_ttl: None,
        max_forwarding_ttl: None,
        fast_mode: true,
    });

    let relay_daemon = Arc::new(RelayDaemon::new(text_config));
    relay_daemon.start().await;

    std::future::pending::<()>().await;
}

struct TextConfig(Config);

impl ReadConfig for TextConfig {
    fn read(&self) -> &Config {
        &self.0
    }
}

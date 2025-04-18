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
    });

    let relay_daemon = Arc::new(RelayDaemon::new_fast(text_config));
    relay_daemon.start().await;

    tokio::signal::ctrl_c()
        .await
        .expect("should be able to wait on ctrl+c");
}

struct TextConfig(Config);

impl ReadConfig for TextConfig {
    fn read(&self) -> Option<&Config> {
        Some(&self.0)
    }
}

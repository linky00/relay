use std::collections::HashSet;

use relay_daemon::{
    config::{Config, GetConfig},
    daemon::Daemon,
    line::GetLine,
};

#[tokio::main]
async fn main() {
    let text_config = TextConfig(Config {
        name: "blah".to_owned(),
        trusted_keys: HashSet::new(),
        contacting_hosts: HashSet::from(["example.com".into()]),
        initial_ttl: None,
        max_forwarding_ttl: None,
    });

    let relay_daemon = Daemon::new(RepeatingLine, text_config).fast();
    relay_daemon.start_sending_to_hosts().await;

    tokio::signal::ctrl_c()
        .await
        .expect("should be able to wait on ctrl+c");
}

struct RepeatingLine;

impl GetLine for RepeatingLine {
    fn get(&self) -> Option<String> {
        Some("blah".to_owned())
    }
}

struct TextConfig(Config);

impl GetConfig for TextConfig {
    fn get(&self) -> Option<&Config> {
        Some(&self.0)
    }
}

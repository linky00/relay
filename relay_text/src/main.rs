use std::env;

use relay_core::{crypto::SecretKey, mailroom::GetNextLine};
use relay_daemon::{
    config::{Config, GetConfig, RelayData},
    daemon::Daemon,
    event::{Event, HandleEvent},
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("should be able to read dotenv");

    let text_config = TextConfig(Config {
        name: "blah".to_owned(),
        secret_key: SecretKey::generate(),
        trusted_relays: vec![
            RelayData::new(
                SecretKey::generate().public_key(),
                Some(&env::var("RELAY_URL").expect("RELAY_URL should be present")),
                Some("another relay".to_owned()),
            )
            .unwrap(),
        ],
        initial_ttl: None,
        max_forwarding_ttl: None,
    });

    let relay_daemon = Daemon::new_fast(IncreasingLine::new(), text_config, EventPrinter);
    relay_daemon.start().await;

    tokio::signal::ctrl_c()
        .await
        .expect("should be able to wait on ctrl+c");
}

struct IncreasingLine {
    count: u32,
}

impl IncreasingLine {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl GetNextLine for IncreasingLine {
    fn get_next_line(&mut self) -> Option<String> {
        self.count += 1;
        let line = format!("line {}", self.count);
        println!("generated new line: \"{line}\"");
        Some(line)
    }
}

struct TextConfig(Config);

impl GetConfig for TextConfig {
    fn get(&self) -> Option<&Config> {
        Some(&self.0)
    }
}

struct EventPrinter;

impl EventPrinter {
    fn relay_display(relay: RelayData) -> String {
        format!("\"{}\"", relay.nickname.unwrap_or(relay.key.to_string()))
    }
}

impl HandleEvent for EventPrinter {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::BeginningSendingToHosts => {
                println!("sending to hosts");
            }
            Event::SentToHost(host, envelopes) => {
                println!(
                    "sent host relay {} {} envelopes",
                    Self::relay_display(host),
                    envelopes.len()
                );
            }
            Event::ReceivedFromHost(host, envelopes) => {
                println!(
                    "received from host relay {} {} envelopes",
                    Self::relay_display(host),
                    envelopes.len()
                );
            }
            Event::ProblemSendingToHost(host, error) => {
                println!(
                    "problem sending to host relay {}: {}",
                    Self::relay_display(host),
                    error
                );
            }
            Event::HttpErrorResponseFromHost(host, error) => {
                println!(
                    "http error response from host relay {}: {}",
                    Self::relay_display(host),
                    error
                );
            }
            Event::BadResponseFromHost(host) => {
                println!(
                    "received bad response from host relay {}",
                    Self::relay_display(host)
                );
            }
            Event::AlreadyReceivedFromHost(host) => {
                println!(
                    "already received from host relay {}",
                    Self::relay_display(host)
                );
            }
            Event::FinishedSendingToHosts => {
                println!("finished sending to hosts");
            }
        }
    }
}

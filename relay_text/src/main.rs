use std::env;

use relay_core::{crypto::SecretKey, mailroom::GetNextLine};
use relay_daemon::{
    config::{Config, GetConfig, ListenerConfig, RelayData},
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
                Some("another relay".to_owned()),
                Some(&env::var("RELAY_URL").expect("RELAY_URL should be present")),
            )
            .unwrap(),
        ],
        custom_initial_ttl: None,
        custom_max_forwarding_ttl: None,
        listener_config: Some(ListenerConfig { custom_port: None }),
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
            Event::BeginningSendingToListeners => {
                println!("sending to listeners");
            }
            Event::SentToListener(relay, envelopes) => {
                println!(
                    "sent listener relay {} {} envelopes",
                    Self::relay_display(relay),
                    envelopes.len()
                );
            }
            Event::ReceivedFromListener(relay, envelopes) => {
                println!(
                    "received from listener relay {} {} envelopes",
                    Self::relay_display(relay),
                    envelopes.len()
                );
            }
            Event::ProblemSendingToListener(relay, error) => {
                println!(
                    "problem sending to listener relay {}: {}",
                    Self::relay_display(relay),
                    error
                );
            }
            Event::HttpErrorResponseFromListener(relay, error) => {
                println!(
                    "http error response from listener relay {}: {}",
                    Self::relay_display(relay),
                    error
                );
            }
            Event::BadResponseFromListener(relay) => {
                println!(
                    "received bad response from listener relay {}",
                    Self::relay_display(relay)
                );
            }
            Event::AlreadyReceivedFromListener(relay) => {
                println!(
                    "already received from listener relay {}",
                    Self::relay_display(relay)
                );
            }
            Event::FinishedSendingToListener => {
                println!("finished sending to listener");
            }
        }
    }
}

use std::env;

use relay_core::{
    crypto::SecretKey,
    mailroom::{GetNextLine, NextLine},
};
use relay_daemon::{
    config::{DaemonConfig, RelayData},
    daemon::Daemon,
    event::{Event, HandleEvent},
};

mod files;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("should be able to read dotenv");

    let secret_key = SecretKey::generate();

    let daemon_config = DaemonConfig {
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
    };

    let relay_daemon = Daemon::new_fast(
        IncreasingLine::new("me"),
        EventPrinter,
        secret_key,
        &env::var("ARCHIVE_DB").unwrap(),
        daemon_config,
    )
    .await
    .unwrap();

    relay_daemon.start_sender().await.unwrap();

    relay_daemon.start_listener(None).await.unwrap();

    tokio::signal::ctrl_c()
        .await
        .expect("should be able to wait on ctrl+c");
}

struct IncreasingLine {
    author: String,
    count: u32,
}

impl IncreasingLine {
    fn new<S: Into<String>>(author: S) -> Self {
        Self {
            author: author.into(),
            count: 0,
        }
    }
}

impl GetNextLine for IncreasingLine {
    fn get_next_line(&mut self) -> Option<NextLine> {
        self.count += 1;
        let text = format!("line {}", self.count);
        println!("generated new line: \"{text}\"");
        Some(NextLine {
            line: text,
            author: self.author.clone(),
        })
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
            Event::ListenerStartedListening(port) => {
                println!("listener started listening on {port}");
            }
            Event::ListenerReceivedFromSender(relay_data, envelopes) => {
                println!(
                    "listener received from sender relay {}: {} envelopes",
                    Self::relay_display(relay_data.expect("this should exist")),
                    envelopes.len()
                );
            }
            Event::ListenerReceivedBadPayload => {
                println!("listener received bad payload");
            }
            Event::ListenerReceivedFromUntrustedSender => {
                println!("listener received from untrusted sender");
            }
            Event::ListenerDBError(error) => {
                println!("listener had db error: {error}");
            }
            Event::ListenerAlreadyReceivedFromSender(relay_data) => {
                println!(
                    "listener already received from sender relay {}",
                    Self::relay_display(relay_data.expect("this should exist"))
                )
            }
            Event::SenderStartedSchedule => {
                println!("sender started schedule");
            }
            Event::SenderBeginningRun => {
                println!("sender beginning run");
            }
            Event::SenderDBError(error) => {
                println!("sender had db error: {error}");
            }
            Event::SenderSentToListener(relay, envelopes) => {
                println!(
                    "sender sent listener relay {}: {} envelopes",
                    Self::relay_display(relay),
                    envelopes.len()
                );
            }
            Event::SenderReceivedFromListener(relay, envelopes) => {
                println!(
                    "sender received from listener relay {}: {} envelopes",
                    Self::relay_display(relay),
                    envelopes.len()
                );
            }
            Event::SenderFailedSending(relay, error) => {
                println!(
                    "sender failed sending to listener relay {}: {}",
                    Self::relay_display(relay),
                    error
                );
            }
            Event::SenderReceivedHttpError(relay, error) => {
                println!(
                    "sender received http error from listener relay {}: {}",
                    Self::relay_display(relay),
                    error
                );
            }
            Event::SenderReceivedBadResponse(relay) => {
                println!(
                    "sender received bad response from listener relay {}",
                    Self::relay_display(relay)
                );
            }
            Event::SenderAlreadyReceivedFromListener(relay) => {
                println!(
                    "sender already received from listener relay {}",
                    Self::relay_display(relay)
                );
            }
            Event::SenderFinishedRun => {
                println!("sender finished run");
            }
            Event::AddedMessageToArchive(message) => {
                println!("adding message to archive: \"{}\"", message.contents.line)
            }
        }
    }
}

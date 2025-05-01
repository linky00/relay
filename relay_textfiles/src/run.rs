use std::path::Path;

use anyhow::Result;
use relay_core::mailroom::{GetNextLine, NextLine};
use relay_daemon::{
    config::{DaemonConfig, RelayData},
    daemon::Daemon,
    event::{Event, HandleEvent},
};

use crate::textfiles::Textfiles;

pub async fn run(dir_path: &Path) -> Result<()> {
    let textfiles = Textfiles::new(dir_path)?;

    let relayt_config = textfiles.read_config()?;
    let poem = textfiles.read_poem()?;

    let daemon_config = DaemonConfig {
        trusted_relays: relayt_config.trusted_relays,
        custom_initial_ttl: relayt_config.initial_ttl,
        custom_max_forwarding_ttl: relayt_config.max_forwarding_ttl,
    };

    let relay_daemon = Daemon::new_fast(
        LineGenerator::new("me", poem),
        EventPrinter,
        textfiles.read_secret()?,
        textfiles.archive_path().as_os_str().try_into()?,
        daemon_config,
    )
    .await
    .unwrap();

    relay_daemon.start_sender().await.unwrap();

    relay_daemon
        .start_listener(relayt_config.listening_port)
        .await
        .unwrap();

    tokio::signal::ctrl_c()
        .await
        .expect("should be able to wait on ctrl+c");

    Ok(())
}

struct LineGenerator {
    author: String,
    poem: Vec<String>,
    n: usize,
}

impl LineGenerator {
    fn new<S: Into<String>>(author: S, poem: Vec<String>) -> Self {
        Self {
            author: author.into(),
            poem,
            n: 0,
        }
    }
}

impl GetNextLine for LineGenerator {
    fn get_next_line(&mut self) -> Option<NextLine> {
        let next_line = if let Some(line) = self.poem.get(self.n) {
            Some(NextLine {
                line: line.to_owned(),
                author: self.author.clone(),
            })
        } else {
            None
        };
        self.n += 1;
        self.n %= self.poem.len();
        next_line
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

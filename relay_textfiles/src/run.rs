use std::path::Path;

use anyhow::Result;
use relay_core::mailroom::{GetNextLine, NextLine};
use relay_daemon::{
    config::{DaemonConfig, RelayData},
    daemon::Daemon,
    event::Event,
};
use tokio::sync::mpsc;

use crate::textfiles::Textfiles;

pub async fn run(dir_path: &Path) -> Result<()> {
    let textfiles = Textfiles::new(dir_path)?;

    let relayt_config = textfiles.read_config()?;
    let poem = textfiles.read_poem()?;

    let line_generator = LineGenerator::new(relayt_config.name, poem);
    let event_printer = EventPrinter::new(textfiles.clone());

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            event_printer.print(event);
        }
    });

    let secret_key = textfiles.read_secret()?;
    let db_url = textfiles.archive_path().as_os_str().try_into()?;
    let daemon_config = DaemonConfig {
        trusted_relays: relayt_config.trusted_relays,
        custom_initial_ttl: relayt_config.initial_ttl,
        custom_max_forwarding_ttl: relayt_config.max_forwarding_ttl,
    };

    let relay_daemon = if textfiles.debug_mode() {
        println!("STARTING IN DEBUG MODE");
        Daemon::new_fast(line_generator, event_tx, secret_key, db_url, daemon_config).await
    } else {
        Daemon::new(line_generator, event_tx, secret_key, db_url, daemon_config).await
    }?;

    relay_daemon.start_sender().await?;

    if relayt_config.listening {
        relay_daemon
            .start_listener(relayt_config.listening_port)
            .await?;
    }

    tokio::signal::ctrl_c().await?;

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

struct EventPrinter {
    textfiles: Textfiles,
}

impl EventPrinter {
    fn new(textfiles: Textfiles) -> Self {
        EventPrinter { textfiles }
    }

    fn print(&self, event: Event) {
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
                println!("adding message to archive: \"{}\"", message.contents.line);
                match self.textfiles.write_listen(&message.contents.line) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("can't write to listen.txt: {e}");
                    }
                };
            }
        }
    }

    fn relay_display(relay: RelayData) -> String {
        format!("\"{}\"", relay.nickname.unwrap_or(relay.key.to_string()))
    }
}

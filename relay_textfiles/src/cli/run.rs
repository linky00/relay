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
            event_printer.print_event(event);
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

enum Source {
    Listener,
    Sender,
    Archive,
}

struct EventPrinter {
    textfiles: Textfiles,
}

impl EventPrinter {
    fn new(textfiles: Textfiles) -> Self {
        EventPrinter { textfiles }
    }

    fn print_event(&self, event: Event) {
        match event {
            Event::ListenerStartedListening(port) => {
                Self::print_from_source(Source::Listener, format!("Started listening on {port}"));
            }
            Event::ListenerReceivedFromSender(relay_data, envelopes) => {
                Self::print_from_source(
                    Source::Listener,
                    format!(
                        "Received {} envelopes from sender relay {}",
                        envelopes.len(),
                        match relay_data {
                            Some(relay_data) => Self::relay_display(relay_data),
                            None => "[unknown relay]".into(),
                        },
                    ),
                );
            }
            Event::ListenerReceivedBadPayload => {
                Self::print_from_source(Source::Listener, format!("Received bad payload"));
            }
            Event::ListenerReceivedFromUntrustedSender => {
                Self::print_from_source(
                    Source::Listener,
                    format!("Received from untrusted sender"),
                );
            }
            Event::ListenerDBError(error) => {
                Self::print_from_source(Source::Listener, format!("Had DB error: {error}"));
            }
            Event::ListenerAlreadyReceivedFromSender(relay_data) => {
                Self::print_from_source(
                    Source::Listener,
                    format!(
                        "Already received from sender relay {}",
                        match relay_data {
                            Some(relay_data) => Self::relay_display(relay_data),
                            None => "[unknown relay]".into(),
                        },
                    ),
                );
            }
            Event::SenderStartedSchedule => {
                Self::print_from_source(Source::Sender, format!("Started schedule"));
            }
            Event::SenderBeginningRun => {
                Self::print_from_source(Source::Sender, format!("Beginning run"));
            }
            Event::SenderDBError(error) => {
                Self::print_from_source(Source::Sender, format!("Had db error: {error}"));
            }
            Event::SenderSentToListener(relay, envelopes) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Sent {} envelopes listener relay {}",
                        envelopes.len(),
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderReceivedFromListener(relay, envelopes) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Received {} envelopes from listener relay {}",
                        envelopes.len(),
                        Self::relay_display(relay),
                    ),
                );
            }
            Event::SenderFailedSending(relay, error) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Failed sending to listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedHttpError(relay, error) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Received http error from listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedBadResponse(relay) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Received bad response from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderAlreadyReceivedFromListener(relay) => {
                Self::print_from_source(
                    Source::Sender,
                    format!(
                        "Already received from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderFinishedRun => {
                Self::print_from_source(Source::Sender, format!("Finished run"));
            }
            Event::AddedMessageToArchive(message) => {
                Self::print_from_source(
                    Source::Archive,
                    format!("Adding message to archive: \"{}\"", message.contents.line),
                );

                match self.textfiles.write_listen(&message.contents.line) {
                    Ok(_) => {}
                    Err(e) => {
                        Self::print_from_source(
                            Source::Archive,
                            format!("Can't write to listen.txt: {e}"),
                        );
                    }
                };
            }
        }
    }

    fn print_from_source(source: Source, line: String) {
        println!(
            "{}{line}",
            match source {
                Source::Listener => "[Listener] ",
                Source::Sender => "[Sender]   ",
                Source::Archive => "[Archive]  ",
            }
        )
    }

    fn relay_display(relay: RelayData) -> String {
        format!("\"{}\"", relay.nickname.unwrap_or(relay.key.to_string()))
    }
}

use std::{fmt::Display, path::Path, sync::Arc};

use anyhow::Result;
use parking_lot::Mutex;
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

    let initial_relayt_config = textfiles.read_config()?;
    let poem = textfiles.read_poem()?;

    let line_generator = LineGenerator::new(initial_relayt_config.name.clone(), poem);
    let author = line_generator.author.clone();

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
        trusted_relays: initial_relayt_config.trusted_relays.clone(),
        custom_initial_ttl: initial_relayt_config.initial_ttl,
        custom_max_forwarding_ttl: initial_relayt_config.max_forwarding_ttl,
    };

    let mut relay_daemon = if textfiles.debug_mode() {
        println!("DEBUG MODE");
        Daemon::new_fast(line_generator, event_tx, secret_key, db_url, daemon_config).await
    } else {
        Daemon::new(line_generator, event_tx, secret_key, db_url, daemon_config).await
    }?;

    relay_daemon.start_sender().await?;

    if let Some(listening_config) = &initial_relayt_config.listener {
        relay_daemon.start_listener(listening_config.port).await?;
    }

    let mut config_change_rx = textfiles.watch_config_changes()?;
    tokio::spawn(async move {
        let mut last_config = initial_relayt_config;
        while let Some(events) = config_change_rx.recv().await {
            if let Ok(_) = events {
                match textfiles.read_config() {
                    Ok(new_config) => {
                        if new_config.name != last_config.name {
                            *author.lock() = new_config.name.clone();
                        }

                        if new_config.trusted_relays != last_config.trusted_relays
                            || new_config.initial_ttl != last_config.initial_ttl
                            || new_config.max_forwarding_ttl != last_config.max_forwarding_ttl
                        {
                            relay_daemon
                                .update_config(DaemonConfig {
                                    trusted_relays: new_config.trusted_relays.clone(),
                                    custom_initial_ttl: new_config.initial_ttl,
                                    custom_max_forwarding_ttl: new_config.max_forwarding_ttl,
                                })
                                .await
                        }

                        if new_config.listener != last_config.listener {
                            print_from_source(
                                Source::Config,
                                "Can't update listener at runtime yet!",
                            );
                        }

                        if last_config != new_config {
                            print_from_source(Source::Config, "Updated config");
                        }

                        last_config = new_config;
                    }
                    Err(e) => {
                        print_from_source(Source::Config, format!("Can't read config: {e}"));
                    }
                }
            }
        }
    });

    tokio::signal::ctrl_c().await?;

    Ok(())
}

struct LineGenerator {
    author: Arc<Mutex<String>>,
    poem: Vec<String>,
    n: usize,
}

impl LineGenerator {
    fn new<S: Into<String>>(author: S, poem: Vec<String>) -> Self {
        Self {
            author: Arc::new(Mutex::new(author.into())),
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
                author: self.author.lock().clone(),
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

    fn print_event(&self, event: Event) {
        match event {
            Event::ListenerStartedListening(port) => {
                print_from_source(Source::Listener, format!("Started listening on {port}"));
            }
            Event::ListenerReceivedFromSender(relay_data, envelopes) => {
                print_from_source(
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
                print_from_source(Source::Listener, "Received bad payload");
            }
            Event::ListenerReceivedFromUntrustedSender => {
                print_from_source(Source::Listener, "Received from untrusted sender");
            }
            Event::ListenerDBError(error) => {
                print_from_source(Source::Listener, format!("Had DB error: {error}"));
            }
            Event::ListenerAlreadyReceivedFromSender(relay_data) => {
                print_from_source(
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
                print_from_source(Source::Sender, "Started schedule");
            }
            Event::SenderBeginningRun => {
                print_from_source(Source::Sender, "Beginning run");
            }
            Event::SenderDBError(error) => {
                print_from_source(Source::Sender, format!("Had db error: {error}"));
            }
            Event::SenderSentToListener(relay, envelopes) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Sent {} envelopes listener relay {}",
                        envelopes.len(),
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderReceivedFromListener(relay, envelopes) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Received {} envelopes from listener relay {}",
                        envelopes.len(),
                        Self::relay_display(relay),
                    ),
                );
            }
            Event::SenderFailedSending(relay, error) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Failed sending to listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedHttpError(relay, error) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Received http error from listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedBadResponse(relay) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Received bad response from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderAlreadyReceivedFromListener(relay) => {
                print_from_source(
                    Source::Sender,
                    format!(
                        "Already received from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderFinishedRun => {
                print_from_source(Source::Sender, "Finished run");
            }
            Event::AddedMessageToArchive(message) => {
                print_from_source(
                    Source::Archive,
                    format!("Adding message to archive: \"{}\"", message.contents.line),
                );

                match self.textfiles.write_listen(&message.contents.line) {
                    Ok(_) => {}
                    Err(e) => {
                        print_from_source(
                            Source::Archive,
                            format!("Can't write to listen.txt: {e}"),
                        );
                    }
                };
            }
        }
    }

    fn relay_display(relay: RelayData) -> String {
        format!("\"{}\"", relay.nickname.unwrap_or(relay.key.to_string()))
    }
}

enum Source {
    Listener,
    Sender,
    Archive,
    Config,
}

fn print_from_source<S: Display>(source: Source, line: S) {
    println!(
        "{}{line}",
        match source {
            Source::Listener => "[Listener] ",
            Source::Sender => "[Sender]   ",
            Source::Archive => "[Archive]  ",
            Source::Config => "[Config]   ",
        }
    )
}

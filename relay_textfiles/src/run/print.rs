use std::{fmt::Display, io::Write};

use relay_daemon::{config::RelayData, event::Event};

use crate::textfiles::Textfiles;

pub struct Printer {
    textfiles: Textfiles,
    interesting_since_last_sender_start: bool,
}

impl Printer {
    pub fn new(textfiles: Textfiles) -> Self {
        Printer {
            textfiles,
            interesting_since_last_sender_start: true,
        }
    }

    pub fn print_event(&mut self, event: Event) {
        match event {
            Event::ListenerStartedListening(port) => {
                self.print_from_source(Source::Listener, format!("Started listening on {port}"));
            }
            Event::ListenerReceivedFromSender(relay_data, envelopes) => {
                if !envelopes.is_empty() {
                    self.print_from_source(
                        Source::Listener,
                        format!(
                            "Received {} envelopes from sender relay {}",
                            envelopes.len(),
                            match relay_data {
                                Some(relay_data) => Self::relay_display(relay_data),
                                None => "[unknown relay]".into(),
                            },
                        ),
                    )
                };
            }
            Event::ListenerSentToSender(relay_data, envelopes) => {
                if !envelopes.is_empty() {
                    self.print_from_source(
                        Source::Listener,
                        format!(
                            "Sent {} envelopes to sender relay {}",
                            envelopes.len(),
                            match relay_data {
                                Some(relay_data) => Self::relay_display(relay_data),
                                None => "[unknown relay]".into(),
                            }
                        ),
                    )
                };
            }
            Event::ListenerReceivedBadPayload => {
                self.print_from_source(Source::Listener, "Received bad payload");
            }
            Event::ListenerReceivedFromUntrustedSender => {
                self.print_from_source(Source::Listener, "Received from untrusted sender");
            }
            Event::ListenerDBError(error) => {
                self.print_from_source(Source::Listener, format!("Had DB error: {error}"));
            }
            Event::ListenerAlreadyReceivedFromSender(relay_data) => {
                self.print_from_source(
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
                self.print_from_source(Source::Sender, "Started schedule");
            }
            Event::SenderBeginningRun => {
                if !self.interesting_since_last_sender_start {
                    print!(".");
                    std::io::stdout().flush().unwrap();
                }
                self.interesting_since_last_sender_start = false;
            }
            Event::SenderDBError(error) => {
                self.print_from_source(Source::Sender, format!("Had db error: {error}"));
            }
            Event::SenderSentToListener(relay, envelopes) => {
                if !envelopes.is_empty() {
                    self.print_from_source(
                        Source::Sender,
                        format!(
                            "Sent {} envelopes to listener relay {}",
                            envelopes.len(),
                            Self::relay_display(relay)
                        ),
                    )
                };
            }
            Event::SenderReceivedFromListener(relay, envelopes) => {
                if !envelopes.is_empty() {
                    self.print_from_source(
                        Source::Sender,
                        format!(
                            "Received {} envelopes from listener relay {}",
                            envelopes.len(),
                            Self::relay_display(relay),
                        ),
                    )
                };
            }
            Event::SenderFailedSending(relay, error) => {
                self.print_from_source(
                    Source::Sender,
                    format!(
                        "Failed sending to listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedHttpError(relay, error) => {
                self.print_from_source(
                    Source::Sender,
                    format!(
                        "Received http error from listener relay {}: {}",
                        Self::relay_display(relay),
                        error
                    ),
                );
            }
            Event::SenderReceivedBadResponse(relay) => {
                self.print_from_source(
                    Source::Sender,
                    format!(
                        "Received bad response from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderAlreadyReceivedFromListener(relay) => {
                self.print_from_source(
                    Source::Sender,
                    format!(
                        "Already received from listener relay {}",
                        Self::relay_display(relay)
                    ),
                );
            }
            Event::SenderFinishedRun => {}
            Event::AddedMessageToArchive(message) => {
                self.print_from_source(
                    Source::Archive,
                    format!("Adding message to archive: \"{}\"", message.contents.line),
                );

                match self.textfiles.write_listen(&message.contents.line) {
                    Ok(_) => {}
                    Err(e) => {
                        self.print_from_source(
                            Source::Archive,
                            format!("Can't write to listen.txt: {e}"),
                        );
                    }
                };
            }
        }
    }

    pub fn print_from_source<D: Display>(&mut self, source: Source, line: D) {
        if !self.interesting_since_last_sender_start {
            println!();
        }
        self.interesting_since_last_sender_start = true;
        println!(
            "{}{line}",
            match source {
                Source::Listener => "[Listener] ",
                Source::Sender => "[Sender]   ",
                Source::Archive => "[Archive]  ",
                Source::Config => "[Config]   ",
                Source::Poem => "[Poem]     ",
            }
        )
    }

    fn relay_display(relay: RelayData) -> String {
        format!("\"{}\"", relay.nickname.unwrap_or(relay.key.to_string()))
    }
}

pub enum Source {
    Listener,
    Sender,
    Archive,
    Config,
    Poem,
}

use relay_daemon::{config::RelayData, event::Event};

use crate::textfiles::Textfiles;

use super::{Source, print_from_source};

pub struct EventPrinter {
    textfiles: Textfiles,
}

impl EventPrinter {
    pub fn new(textfiles: Textfiles) -> Self {
        EventPrinter { textfiles }
    }

    pub fn print_event(&self, event: Event) {
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
            Event::ListenerSentToSender(relay_data, envelopes) => {
                print_from_source(
                    Source::Listener,
                    format!(
                        "Sent {} envelopes to sender relay {}",
                        envelopes.len(),
                        match relay_data {
                            Some(relay_data) => Self::relay_display(relay_data),
                            None => "[unknown relay]".into(),
                        }
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
                        "Sent {} envelopes to listener relay {}",
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

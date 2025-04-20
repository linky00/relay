use std::sync::Arc;

use relay_core::{
    mailroom::{self, Archive, Mailroom, OutgoingConfig, TTLConfig},
    payload::UntrustedPayload,
};
use reqwest::Client;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    event::{Event, HandleEvent},
};

pub async fn send_to_hosts<A, E>(
    mailroom: Arc<Mutex<Mailroom<A>>>,
    line: Option<String>,
    config: &Config,
    event_handler: Arc<Mutex<E>>,
    fast_mode: bool,
) where
    A: Archive + Send + 'static,
    E: HandleEvent + Send + 'static,
{
    let client = Client::new();

    let outgoing_config = create_outgoing_config(&config);

    let _ = config
        .trusted_relays
        .iter()
        .filter_map(|relay| match &relay.host {
            Some(host) => Some((relay.clone(), host.clone())),
            None => None,
        })
        .map(async |(relay, host)| {
            let outgoing_envelopes =
                mailroom
                    .blocking_lock()
                    .get_outgoing(&relay.key, line.clone(), &outgoing_config);

            let outgoing_payload = outgoing_envelopes.create_payload();

            let client = client.clone();
            let mailroom = Arc::clone(&mailroom);
            let config = config.clone();
            let event_handler = Arc::clone(&event_handler);

            tokio::spawn(async move {
                match client.post(host).body(outgoing_payload).send().await {
                    Ok(response) => {
                        let mut event_handler = event_handler.lock().await;

                        event_handler.handle_event(Event::SentToHost(
                            relay.clone(),
                            outgoing_envelopes.envelopes,
                        ));

                        if response.status().is_success() {
                            match response.text().await {
                                Ok(response_text) => {
                                    match UntrustedPayload::from_json(&response_text) {
                                        Ok(untrusted_payload) => match untrusted_payload
                                            .try_trust(config.trusted_public_keys())
                                        {
                                            Ok(trusted_payload) => {
                                                event_handler.handle_event(
                                                    Event::ReceivedFromHost(
                                                        relay.clone(),
                                                        trusted_payload.envelopes().clone(),
                                                    ),
                                                );

                                                if mailroom
                                                    .lock()
                                                    .await
                                                    .receive_payload(trusted_payload)
                                                    .is_err()
                                                {
                                                    event_handler.handle_event(
                                                        Event::AlreadyReceivedFromHost(
                                                            relay.clone(),
                                                        ),
                                                    );
                                                }
                                            }
                                            Err(_) => {
                                                event_handler.handle_event(
                                                    Event::BadResponseFromHost(relay.clone()),
                                                );
                                            }
                                        },
                                        Err(_) => {
                                            event_handler.handle_event(Event::BadResponseFromHost(
                                                relay.clone(),
                                            ));
                                        }
                                    }
                                }
                                Err(_) => {
                                    event_handler
                                        .handle_event(Event::BadResponseFromHost(relay.clone()));
                                }
                            }
                        } else {
                            event_handler.handle_event(Event::HttpErrorResponseFromHost(
                                relay.clone(),
                                format!(
                                    "{}: {}",
                                    response.status().as_u16(),
                                    response.status().canonical_reason().unwrap_or_default()
                                ),
                            ));
                        }
                    }
                    Err(error) => {
                        event_handler
                            .lock()
                            .await
                            .handle_event(Event::ProblemSendingToHost(
                                relay.clone(),
                                error.to_string(),
                            ));
                    }
                };
            })
        });
}

fn create_outgoing_config(config: &Config) -> OutgoingConfig {
    OutgoingConfig::new(
        config.name.clone(),
        config.secret_key.clone(),
        TTLConfig::new(config.initial_ttl, config.max_forwarding_ttl),
    )
}

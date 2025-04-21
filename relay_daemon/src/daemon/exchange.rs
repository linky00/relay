use std::sync::Arc;

use futures::future;
use relay_core::{
    mailroom::{Archive, GetNextLine, Mailroom, OutgoingConfig, TTLConfig},
    payload::UntrustedPayload,
};
use reqwest::Client;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    event::{self, Event, HandleEvent},
};

pub async fn send_to_hosts<L, A, E>(
    mailroom: Arc<Mutex<Mailroom<L, A>>>,
    config: &Config,
    event_handler: Arc<Mutex<E>>,
) where
    L: GetNextLine + Send + 'static,
    A: Archive + Send + 'static,
    E: HandleEvent + Send + 'static,
{
    event::emit_event(&event_handler, Event::BeginningSendingToHosts).await;

    let client = Client::new();

    let outgoing_config = create_outgoing_config(&config);

    let handles: Vec<_> = config
        .trusted_relays
        .iter()
        .filter_map(|relay| match &relay.host {
            Some(host) => Some((relay.clone(), host.clone())),
            None => None,
        })
        .map(|(relay, host)| {
            let client = client.clone();
            let mailroom = Arc::clone(&mailroom);
            let config = config.clone();
            let event_handler = Arc::clone(&event_handler);
            let outgoing_config = outgoing_config.clone();

            async move {
                let outgoing_envelopes = mailroom
                    .lock()
                    .await
                    .get_outgoing(&relay.key, &outgoing_config);

                let outgoing_payload = outgoing_envelopes.create_payload();

                match client.post(host).body(outgoing_payload).send().await {
                    Ok(response) => {
                        event::emit_event(
                            &event_handler,
                            Event::SentToHost(relay.clone(), outgoing_envelopes.envelopes),
                        )
                        .await;

                        let handle_response = async || {
                            if !response.status().is_success() {
                                return Err(Event::HttpErrorResponseFromHost(
                                    relay.clone(),
                                    format!(
                                        "{}: {}",
                                        response.status().as_u16(),
                                        response.status().canonical_reason().unwrap_or_default()
                                    ),
                                ));
                            }

                            let response_text = response
                                .text()
                                .await
                                .map_err(|_| Event::BadResponseFromHost(relay.clone()))?;

                            let untrusted_payload = UntrustedPayload::from_json(&response_text)
                                .map_err(|_| Event::BadResponseFromHost(relay.clone()))?;

                            let trusted_payload = untrusted_payload
                                .try_trust(config.trusted_public_keys())
                                .map_err(|_| Event::BadResponseFromHost(relay.clone()))?;

                            let envelopes = trusted_payload.envelopes().clone();

                            match mailroom.lock().await.receive_payload(trusted_payload) {
                                Ok(_) => Ok(Event::ReceivedFromHost(relay.clone(), envelopes)),
                                Err(_) => Ok(Event::AlreadyReceivedFromHost(relay.clone())),
                            }
                        };

                        {
                            match handle_response().await {
                                Ok(event) => {
                                    event::emit_event(&event_handler, event).await;
                                }
                                Err(event) => {
                                    event::emit_event(&event_handler, event).await;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        event::emit_event(
                            &event_handler,
                            Event::ProblemSendingToHost(relay.clone(), error.to_string()),
                        )
                        .await;
                    }
                };
            }
        })
        .collect();

    future::join_all(handles).await;

    event::emit_event(&event_handler, Event::FinishedSendingToHosts).await;
}

fn create_outgoing_config(config: &Config) -> OutgoingConfig {
    OutgoingConfig::new(
        config.name.clone(),
        config.secret_key.clone(),
        TTLConfig::new(config.initial_ttl, config.max_forwarding_ttl),
    )
}

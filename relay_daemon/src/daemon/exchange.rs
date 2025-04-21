use std::sync::Arc;

use futures::future;
use relay_core::{
    mailroom::{Archive, GetNextLine, Mailroom, TTLConfig},
    payload::UntrustedPayload,
};
use reqwest::Client;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    event::{self, Event, HandleEvent},
};

pub async fn send_to_listeners<L, A, E>(
    mailroom: Arc<Mutex<Mailroom<L, A>>>,
    config: &Config,
    event_handler: Arc<Mutex<E>>,
) where
    L: GetNextLine + Send + 'static,
    A: Archive + Send + 'static,
    E: HandleEvent + Send + 'static,
{
    event::emit_event(&event_handler, Event::BeginningSendingToListeners).await;

    let client = Client::new();

    let ttl_config = create_ttl_config(config);

    let handles: Vec<_> = config
        .trusted_relays
        .iter()
        .filter_map(|relay| {
            relay
                .listener_endpoint
                .as_ref()
                .map(|endpoint| (relay.clone(), endpoint.clone()))
        })
        .map(|(relay, endpoint)| {
            let client = client.clone();
            let mailroom = Arc::clone(&mailroom);
            let config = config.clone();
            let event_handler = Arc::clone(&event_handler);

            async move {
                let outgoing_envelopes = mailroom.lock().await.get_outgoing(&relay.key, ttl_config);

                let outgoing_payload = outgoing_envelopes.create_payload();

                match client
                    .post(endpoint)
                    .header(CONTENT_TYPE, "application/json")
                    .body(outgoing_payload)
                    .send()
                    .await
                {
                    Ok(response) => {
                        event::emit_event(
                            &event_handler,
                            Event::SentToListener(relay.clone(), outgoing_envelopes.envelopes),
                        )
                        .await;

                        let handle_response = async || {
                            if !response.status().is_success() {
                                return Err(Event::HttpErrorResponseFromListener(
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
                                .map_err(|_| Event::BadResponseFromListener(relay.clone()))?;

                            let untrusted_payload = UntrustedPayload::from_json(&response_text)
                                .map_err(|_| Event::BadResponseFromListener(relay.clone()))?;

                            let trusted_payload = untrusted_payload
                                .try_trust(config.trusted_public_keys())
                                .map_err(|_| Event::BadResponseFromListener(relay.clone()))?;

                            let envelopes = trusted_payload.envelopes().clone();

                            match mailroom.lock().await.receive_payload(trusted_payload) {
                                Ok(()) => Ok(Event::ReceivedFromListener(relay.clone(), envelopes)),
                                Err(_) => Ok(Event::AlreadyReceivedFromListener(relay.clone())),
                            }
                        };

                        let event = handle_response().await.unwrap_or_else(|e| e);
                        event::emit_event(&event_handler, event).await;
                    }
                    Err(error) => {
                        event::emit_event(
                            &event_handler,
                            Event::ProblemSendingToListener(relay.clone(), error.to_string()),
                        )
                        .await;
                    }
                }
            }
        })
        .collect();

    future::join_all(handles).await;

    event::emit_event(&event_handler, Event::FinishedSendingToListeners).await;
}

fn create_ttl_config(config: &Config) -> TTLConfig {
    TTLConfig::new(config.custom_initial_ttl, config.custom_max_forwarding_ttl)
}

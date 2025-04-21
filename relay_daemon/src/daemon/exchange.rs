use std::sync::Arc;

use futures::future;
use relay_core::{
    mailroom::{Archive, Mailroom, OutgoingConfig, TTLConfig},
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
    event_handler
        .lock()
        .await
        .handle_event(Event::SendingToHosts);

    let client = Client::new();

    let outgoing_config = create_outgoing_config(&config);

    let handles = config
        .trusted_relays
        .iter()
        .filter_map(|relay| match &relay.host {
            Some(host) => Some((relay.clone(), host.clone())),
            None => None,
        })
        .map(async |(relay, host)| {
            let outgoing_envelopes =
                mailroom
                    .lock()
                    .await
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

                        match handle_response().await {
                            Ok(event) => {
                                event_handler.handle_event(event);
                            }
                            Err(event) => {
                                event_handler.handle_event(event);
                            }
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

    future::join_all(handles).await;

    event_handler
        .lock()
        .await
        .handle_event(Event::FinishedSendingToHosts);
}

fn create_outgoing_config(config: &Config) -> OutgoingConfig {
    OutgoingConfig::new(
        config.name.clone(),
        config.secret_key.clone(),
        TTLConfig::new(config.initial_ttl, config.max_forwarding_ttl),
    )
}

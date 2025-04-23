use std::sync::Arc;

use axum::http::StatusCode;
use chrono::Utc;
use futures::future;
use relay_core::{
    mailroom::{GetNextLine, Mailroom, MailroomError, TTLConfig},
    payload::UntrustedPayload,
};
use reqwest::{Client, header::CONTENT_TYPE};
use tokio::sync::Mutex;

use crate::{
    config::{Config, GetConfig},
    event::{self, Event, HandleEvent},
};

use super::archive::{DBArchive, DBError};

pub async fn send_to_listeners<L, E>(
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    config: &Config,
    event_handler: Arc<Mutex<E>>,
) where
    L: GetNextLine + Send + 'static,
    E: HandleEvent + Send + 'static,
{
    event::emit_event(&event_handler, Event::SenderBeginningRun).await;

    let now = Utc::now();
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
                let outgoing_envelopes = match mailroom
                    .lock()
                    .await
                    .get_outgoing_at_time(&relay.key, ttl_config, now)
                    .await
                {
                    Ok(outgoing_envelopes) => outgoing_envelopes,
                    Err(error) => {
                        event::emit_event(&event_handler, Event::SenderDBError(error.to_string()));
                        return;
                    }
                };

                match client
                    .post(endpoint)
                    .header(CONTENT_TYPE, "application/json")
                    .body(outgoing_envelopes.create_payload())
                    .send()
                    .await
                {
                    Ok(response) => {
                        event::emit_event(
                            &event_handler,
                            Event::SenderSentToListener(
                                relay.clone(),
                                outgoing_envelopes.envelopes,
                            ),
                        )
                        .await;

                        let handle_response = async || {
                            if !response.status().is_success() {
                                return Err(Event::SenderReceivedHttpError(
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
                                .map_err(|_| Event::SenderReceivedBadResponse(relay.clone()))?;

                            let untrusted_payload = UntrustedPayload::from_json(&response_text)
                                .map_err(|_| Event::SenderReceivedBadResponse(relay.clone()))?;

                            let trusted_payload = untrusted_payload
                                .try_trust(config.trusted_public_keys())
                                .map_err(|_| Event::SenderReceivedBadResponse(relay.clone()))?;

                            match mailroom
                                .lock()
                                .await
                                .receive_payload_at_time(&trusted_payload, now)
                                .await
                            {
                                Ok(()) => Ok(Event::SenderReceivedFromListener(
                                    relay.clone(),
                                    trusted_payload.envelopes().clone(),
                                )),
                                Err(MailroomError::AlreadyReceivedFromKey) => {
                                    Ok(Event::SenderAlreadyReceivedFromListener(relay.clone()))
                                }
                                Err(MailroomError::ArchiveFailure(error)) => {
                                    Ok(Event::SenderDBError(error.to_string()))
                                }
                            }
                        };

                        let event = handle_response().await.unwrap_or_else(|e| e);
                        event::emit_event(&event_handler, event).await;
                    }
                    Err(error) => {
                        event::emit_event(
                            &event_handler,
                            Event::SenderFailedSending(relay.clone(), error.to_string()),
                        )
                        .await;
                    }
                }
            }
        })
        .collect();

    future::join_all(handles).await;

    event::emit_event(&event_handler, Event::SenderFinishedRun).await;
}

pub async fn respond_to_sender<L, C, E>(
    payload: &str,
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    config_reader: Arc<C>,
    event_handler: Arc<Mutex<E>>,
) -> Result<String, (StatusCode, String)>
where
    L: GetNextLine,
    C: GetConfig,
    E: HandleEvent + Send + 'static,
{
    let now = Utc::now();

    // todo: config code smell eww
    let config = {
        match config_reader.get() {
            Some(config) => config.clone(),
            None => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, "sorry".to_owned()));
            }
        }
    };

    let trusted_payload = match UntrustedPayload::from_json(payload) {
        Ok(untrusted_payload) => match untrusted_payload.try_trust(config.trusted_public_keys()) {
            Ok(trusted_payload) => trusted_payload,
            Err(_) => {
                event::emit_event(&event_handler, Event::ListenerReceivedFromUntrustedSender).await;
                return Err((
                    StatusCode::FORBIDDEN,
                    "payload certificate key not trusted".to_owned(),
                ));
            }
        },
        Err(_) => {
            event::emit_event(&event_handler, Event::ListenerReceivedBadPayload).await;
            return Err((StatusCode::BAD_REQUEST, "payload malformed".to_owned()));
        }
    };

    let relay_data = config
        .trusted_relays
        .iter()
        .find(|relay| relay.key.to_string() == trusted_payload.certificate().key)
        .cloned();

    let mut mailroom = mailroom.lock().await;

    match mailroom
        .receive_payload_at_time(&trusted_payload, now)
        .await
    {
        Ok(()) => {
            event::emit_event(
                &event_handler,
                Event::ListenerReceivedFromSender(relay_data, trusted_payload.envelopes().clone()),
            )
            .await;

            let outgoing_envelopes = mailroom.get_outgoing_at_time(
                &trusted_payload.public_key(),
                create_ttl_config(&config),
                now,
            );

            match outgoing_envelopes.await {
                Ok(outgoing_envelopes) => Ok(outgoing_envelopes.create_payload()),
                Err(error) => {
                    event::emit_event(&event_handler, Event::ListenerDBError(error.to_string()));
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "db error sorry".to_owned(),
                    ))
                }
            }
        }
        Err(MailroomError::AlreadyReceivedFromKey) => {
            event::emit_event(
                &event_handler,
                Event::ListenerAlreadyReceivedFromSender(relay_data),
            )
            .await;
            Err((
                StatusCode::FORBIDDEN,
                "already received payload with this certificate key this period".to_owned(),
            ))
        }
        Err(MailroomError::ArchiveFailure(error)) => {
            event::emit_event(&event_handler, Event::ListenerDBError(error.to_string()));
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "db error sorry".to_owned(),
            ))
        }
    }
}

fn create_ttl_config(config: &Config) -> TTLConfig {
    TTLConfig::new(config.custom_initial_ttl, config.custom_max_forwarding_ttl)
}

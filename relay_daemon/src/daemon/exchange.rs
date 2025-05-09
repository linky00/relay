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
    config::DaemonConfig,
    event::{Event, EventSender},
};

use super::archive::{DBArchive, DBError};

pub async fn send_to_listeners<L>(
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    config: &DaemonConfig,
    event_sender: EventSender,
) where
    L: GetNextLine + Send + 'static,
{
    event_sender.send(Event::SenderBeginningRun).ok();

    let now = Utc::now();
    let client = Client::new();
    let ttl_config = create_ttl_config(config);

    let handles: Vec<_> = config
        .trusted_relays
        .iter()
        .filter_map(|relay| relay.endpoint.as_ref().map(|endpoint| (relay, endpoint)))
        .map(|(relay, endpoint)| {
            let client = client.clone();
            let mailroom = Arc::clone(&mailroom);
            let config = config.clone();
            let event_sender = event_sender.clone();

            async move {
                let outgoing_envelopes = match mailroom
                    .lock()
                    .await
                    .get_outgoing_at_time(&relay.key, ttl_config, now)
                    .await
                {
                    Ok(outgoing_envelopes) => outgoing_envelopes,
                    Err(error) => {
                        event_sender
                            .send(Event::SenderDBError(error.to_string()))
                            .ok();
                        return;
                    }
                };

                match client
                    .post(endpoint.clone())
                    .header(CONTENT_TYPE, "application/json")
                    .body(outgoing_envelopes.create_payload())
                    .send()
                    .await
                {
                    Ok(response) => {
                        event_sender
                            .send(Event::SenderSentToListener(
                                relay.clone(),
                                outgoing_envelopes.envelopes,
                            ))
                            .ok();

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
                        event_sender.send(event).ok();
                    }
                    Err(error) => {
                        event_sender
                            .send(Event::SenderFailedSending(relay.clone(), error.to_string()))
                            .ok();
                    }
                }
            }
        })
        .collect();

    future::join_all(handles).await;

    event_sender.send(Event::SenderFinishedRun).ok();
}

pub async fn respond_to_sender<L>(
    payload: &str,
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    config: &DaemonConfig,
    event_sender: EventSender,
) -> Result<String, (StatusCode, String)>
where
    L: GetNextLine,
{
    let now = Utc::now();

    let trusted_payload = match UntrustedPayload::from_json(payload) {
        Ok(untrusted_payload) => match untrusted_payload.try_trust(config.trusted_public_keys()) {
            Ok(trusted_payload) => trusted_payload,
            Err(_) => {
                event_sender
                    .send(Event::ListenerReceivedFromUntrustedSender)
                    .ok();
                return Err((
                    StatusCode::FORBIDDEN,
                    "payload certificate key not trusted".to_owned(),
                ));
            }
        },
        Err(_) => {
            event_sender.send(Event::ListenerReceivedBadPayload).ok();
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
            event_sender
                .send(Event::ListenerReceivedFromSender(
                    relay_data.clone(),
                    trusted_payload.envelopes().clone(),
                ))
                .ok();

            let outgoing_envelopes = mailroom.get_outgoing_at_time(
                &trusted_payload.public_key(),
                create_ttl_config(&config),
                now,
            );

            match outgoing_envelopes.await {
                Ok(outgoing_envelopes) => {
                    event_sender
                        .send(Event::ListenerSentToSender(
                            relay_data,
                            outgoing_envelopes.envelopes.clone(),
                        ))
                        .ok();
                    Ok(outgoing_envelopes.create_payload())
                }
                Err(error) => {
                    event_sender
                        .send(Event::ListenerDBError(error.to_string()))
                        .ok();
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "db error sorry".to_owned(),
                    ))
                }
            }
        }
        Err(MailroomError::AlreadyReceivedFromKey) => {
            event_sender
                .send(Event::ListenerAlreadyReceivedFromSender(relay_data))
                .ok();
            Err((
                StatusCode::FORBIDDEN,
                "already received payload with this certificate key this period".to_owned(),
            ))
        }
        Err(MailroomError::ArchiveFailure(error)) => {
            event_sender
                .send(Event::ListenerDBError(error.to_string()))
                .ok();
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "db error sorry".to_owned(),
            ))
        }
    }
}

fn create_ttl_config(config: &DaemonConfig) -> TTLConfig {
    TTLConfig::new(config.custom_initial_ttl, config.custom_max_forwarding_ttl)
}

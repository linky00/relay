use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Timelike, Utc};
use thiserror::Error;

use crate::{
    crypto::{PublicKey, SecretKey},
    message::{Envelope, Message, RelayID},
    payload::VerifiedPayload,
};

const DEFAULT_INITIAL_TTL: u8 = 8;
const DEFAULT_MAX_FORWARDING_TTL: u8 = 8;

#[derive(Error, Debug)]
pub enum ReceivePayloadError {
    #[error("already received payload from this key")]
    AlreadyReceivedFromKey,
}

#[derive(Error, Debug)]
pub enum CreateOutgoingEnvelopesError {}

pub struct Mailroom<A: Archive> {
    new_messages: HashSet<Message>,
    forwarding_received_this_hour: HashMap<PublicKey, Vec<Envelope>>,
    forwarding_received_last_hour: HashMap<PublicKey, Vec<Envelope>>,
    last_seen_time: Option<DateTime<Utc>>,
    config: MailroomConfig,
    archive: A,
}

impl<A: Archive> Mailroom<A> {
    pub fn new(config: MailroomConfig, archive: A) -> Self {
        Mailroom {
            new_messages: HashSet::new(),
            forwarding_received_this_hour: HashMap::new(),
            forwarding_received_last_hour: HashMap::new(),
            last_seen_time: None,
            config,
            archive,
        }
    }

    pub fn receive_payload(&mut self, payload: VerifiedPayload) -> Result<(), ReceivePayloadError> {
        self.receive_payload_at_time(payload, Utc::now())
    }

    #[cfg(feature = "chrono")]
    pub fn receive_payload_at_time(
        &mut self,
        payload: VerifiedPayload,
        now: DateTime<Utc>,
    ) -> Result<(), ReceivePayloadError> {
        self.handle_time(now);

        if self
            .forwarding_received_this_hour
            .contains_key(&payload.public_key)
        {
            return Err(ReceivePayloadError::AlreadyReceivedFromKey);
        }

        let mut forwarding_from_this_key = vec![];

        for envelope in payload.envelopes {
            self.archive
                .add_envelope_to_archive(&payload.from, &envelope);

            if self.new_messages.contains(&envelope.message) {
                forwarding_from_this_key.push(envelope);
            } else if !self.archive.is_message_in_archive(&envelope.message) {
                self.new_messages.insert(envelope.message.clone());
                forwarding_from_this_key.push(envelope);
            }
        }

        self.forwarding_received_this_hour
            .insert(payload.public_key, forwarding_from_this_key);

        Ok(())
    }

    pub fn get_outgoing(&mut self, sending_to: &PublicKey, line: String) -> OutgoingEnvelopes {
        self.get_outgoing_at_time(sending_to, line, Utc::now())
    }

    #[cfg(feature = "chrono")]
    pub fn get_outgoing_at_time(
        &mut self,
        sending_to: &PublicKey,
        line: String,
        now: DateTime<Utc>,
    ) -> OutgoingEnvelopes {
        self.handle_time(now);

        let mut sending_envelopes: Vec<Envelope> = self
            .forwarding_received_last_hour
            .iter()
            .filter(|(from_key, _)| *from_key != sending_to)
            .flat_map(|(_, envelopes)| envelopes.iter().cloned())
            .filter_map(|mut envelope| {
                envelope.ttl -= self
                    .config
                    .ttl_config
                    .max_forwarding_ttl
                    .min(envelope.ttl - 1);
                if envelope.ttl > 0 {
                    Some(envelope)
                } else {
                    None
                }
            })
            .collect();

        sending_envelopes.push(Envelope {
            forwarded: vec![],
            ttl: self.config.ttl_config.initial_ttl,
            message: Message::new(line, self.config.relay_id.clone()),
        });

        OutgoingEnvelopes {
            envelopes: sending_envelopes,
            relay_id: self.config.relay_id.clone(),
            secret_key: self.config.secret_key.clone(),
        }
    }

    fn handle_time(&mut self, now: DateTime<Utc>) {
        if let Some(last_seen_time) = self.last_seen_time {
            let on_the_hour = |datetime: DateTime<Utc>| {
                datetime
                    .with_minute(0)
                    .expect("should be able to set any utc time to minute 0")
                    .with_second(0)
                    .expect("should be able to set any utc time to second 0")
                    .with_nanosecond(0)
                    .expect("should be able to set any utc time to nanosecond 0")
            };

            let now_oth = on_the_hour(now);
            let last_seen_oth = on_the_hour(last_seen_time);

            if now_oth == last_seen_oth + Duration::hours(1) {
                self.new_messages = HashSet::new();
                self.forwarding_received_last_hour = self.forwarding_received_this_hour.clone();
                self.forwarding_received_this_hour = HashMap::new();
            } else if now_oth != last_seen_oth {
                self.new_messages = HashSet::new();
                self.forwarding_received_last_hour = HashMap::new();
                self.forwarding_received_this_hour = HashMap::new();
            }
        }
        self.last_seen_time = Some(now);
    }
}

pub struct OutgoingEnvelopes {
    pub envelopes: Vec<Envelope>,
    pub(crate) relay_id: RelayID,
    pub(crate) secret_key: SecretKey,
}

pub struct MailroomConfig {
    pub(crate) relay_id: RelayID,
    pub(crate) secret_key: SecretKey,
    pub(crate) ttl_config: TTLConfig,
}

impl MailroomConfig {
    pub fn new(name: String, secret_key: SecretKey, ttl_config: TTLConfig) -> Self {
        let relay_id = RelayID {
            key: secret_key.public_key().to_string(),
            name,
        };

        Self {
            relay_id,
            secret_key,
            ttl_config,
        }
    }
}

pub struct TTLConfig {
    pub initial_ttl: u8,
    pub max_forwarding_ttl: u8,
}

impl TTLConfig {
    pub fn default() -> TTLConfig {
        TTLConfig {
            initial_ttl: DEFAULT_INITIAL_TTL,
            max_forwarding_ttl: DEFAULT_MAX_FORWARDING_TTL,
        }
    }
}

pub trait Archive {
    fn is_message_in_archive(&self, message: &Message) -> bool;

    fn add_envelope_to_archive(&mut self, from: &RelayID, envelope: &Envelope);
}

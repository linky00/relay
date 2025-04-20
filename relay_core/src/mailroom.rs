use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Timelike, Utc};
use thiserror::Error;

use crate::{
    crypto::{PublicKey, SecretKey, get_canon_json_bytes},
    message::{Certificate, Envelope, Message, MessageContents},
    payload::TrustedPayload,
};

const DEFAULT_INITIAL_TTL: u8 = 8;
const DEFAULT_MAX_FORWARDING_TTL: u8 = 8;

#[derive(Error, Debug)]
pub enum ReceivePayloadError {
    #[error("already received payload from this key")]
    AlreadyReceivedFromKey,
}

pub struct Mailroom<A: Archive> {
    new_messages: HashSet<Message>,
    forwarding_received_this_hour: HashMap<PublicKey, Vec<Envelope>>,
    forwarding_received_last_hour: HashMap<PublicKey, Vec<Envelope>>,
    last_seen_time: Option<DateTime<Utc>>,
    archive: A,
    flatten_time: fn(DateTime<Utc>) -> DateTime<Utc>,
    interval: Duration,
}

impl<A: Archive> Mailroom<A> {
    pub fn new(archive: A) -> Self {
        let flatten_time = |datetime: DateTime<Utc>| {
            datetime
                .with_minute(0)
                .expect("should be able to set any utc time to minute 0")
                .with_second(0)
                .expect("should be able to set any utc time to second 0")
                .with_nanosecond(0)
                .expect("should be able to set any utc time to nanosecond 0")
        };

        Mailroom {
            new_messages: HashSet::new(),
            forwarding_received_this_hour: HashMap::new(),
            forwarding_received_last_hour: HashMap::new(),
            last_seen_time: None,
            archive,
            flatten_time,
            interval: Duration::hours(1),
        }
    }

    #[cfg(feature = "chrono")]
    pub fn new_with_custom_time(
        archive: A,
        flatten_time: fn(DateTime<Utc>) -> DateTime<Utc>,
        interval: Duration,
    ) -> Mailroom<A> {
        Mailroom {
            new_messages: HashSet::new(),
            forwarding_received_this_hour: HashMap::new(),
            forwarding_received_last_hour: HashMap::new(),
            last_seen_time: None,
            archive,
            flatten_time,
            interval,
        }
    }

    pub fn receive_payload(&mut self, payload: TrustedPayload) -> Result<(), ReceivePayloadError> {
        self.receive_payload_internal(payload, Utc::now())
    }

    #[cfg(feature = "chrono")]
    pub fn receive_payload_at_time(
        &mut self,
        payload: TrustedPayload,
        now: DateTime<Utc>,
    ) -> Result<(), ReceivePayloadError> {
        self.receive_payload_internal(payload, now)
    }

    fn receive_payload_internal(
        &mut self,
        payload: TrustedPayload,
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
                .add_envelope_to_archive(&payload.certificate.key, &envelope);

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

    pub fn get_outgoing<S: Into<String>>(
        &mut self,
        sending_to: &PublicKey,
        line: Option<S>,
        outgoing_config: &OutgoingConfig,
    ) -> OutgoingEnvelopes {
        self.get_outgoing_internal(sending_to, line, outgoing_config, Utc::now())
    }

    #[cfg(feature = "chrono")]
    pub fn get_outgoing_at_time<S: Into<String>>(
        &mut self,
        sending_to: &PublicKey,
        line: Option<S>,
        outgoing_config: &OutgoingConfig,
        now: DateTime<Utc>,
    ) -> OutgoingEnvelopes {
        self.get_outgoing_internal(sending_to, line, outgoing_config, now)
    }

    fn get_outgoing_internal<S: Into<String>>(
        &mut self,
        sending_to: &PublicKey,
        line: Option<S>,
        outgoing_config: &OutgoingConfig,
        now: DateTime<Utc>,
    ) -> OutgoingEnvelopes {
        self.handle_time(now);

        let mut sending_envelopes: Vec<Envelope> = self
            .forwarding_received_last_hour
            .iter()
            .filter(|(from_key, _)| *from_key != sending_to)
            .flat_map(|(_, envelopes)| envelopes.iter().cloned())
            .filter_map(|mut envelope| {
                envelope.ttl -= outgoing_config
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

        if let Some(line) = line {
            let contents = MessageContents {
                uuid: uuid::Uuid::new_v4().hyphenated().to_string(),
                author: outgoing_config.author.clone(),
                line: line.into(),
            };

            let contents_json = serde_json::to_string(&contents)
                .expect("should be able to serialize any message contents to json");

            let contents_bytes = get_canon_json_bytes(&contents_json)
                .expect("should be able to get canon bytes for any json string");

            let signature = outgoing_config.secret_key.clone().sign(&contents_bytes);

            let envelope = Envelope {
                forwarded: vec![],
                ttl: outgoing_config.ttl_config.initial_ttl,
                message: Message {
                    certificate: Certificate {
                        key: outgoing_config.secret_key.public_key().to_string(),
                        signature,
                    },
                    contents,
                },
            };

            self.archive
                .add_envelope_to_archive(&envelope.message.certificate.key, &envelope);

            sending_envelopes.push(envelope);
        }

        OutgoingEnvelopes {
            envelopes: sending_envelopes,
            secret_key: outgoing_config.secret_key.clone(),
        }
    }

    fn handle_time(&mut self, now: DateTime<Utc>) {
        if let Some(last_seen_time) = self.last_seen_time {
            let now_oth = (self.flatten_time)(now);
            let last_seen_oth = (self.flatten_time)(last_seen_time);

            if now_oth == last_seen_oth + self.interval {
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
    pub(crate) secret_key: SecretKey,
}

pub struct OutgoingConfig {
    pub(crate) author: String,
    pub(crate) secret_key: SecretKey,
    pub(crate) ttl_config: TTLConfig,
}

impl OutgoingConfig {
    pub fn new<S: Into<String>>(author: S, secret_key: SecretKey, ttl_config: TTLConfig) -> Self {
        Self {
            author: author.into(),
            secret_key,
            ttl_config,
        }
    }
}

pub struct TTLConfig {
    initial_ttl: u8,
    max_forwarding_ttl: u8,
}

impl TTLConfig {
    pub fn new(initial_ttl: Option<u8>, max_forwarding_ttl: Option<u8>) -> Self {
        Self {
            initial_ttl: initial_ttl.unwrap_or(DEFAULT_INITIAL_TTL),
            max_forwarding_ttl: max_forwarding_ttl.unwrap_or(DEFAULT_MAX_FORWARDING_TTL),
        }
    }
}

impl Default for TTLConfig {
    fn default() -> Self {
        Self {
            initial_ttl: DEFAULT_INITIAL_TTL,
            max_forwarding_ttl: DEFAULT_MAX_FORWARDING_TTL,
        }
    }
}

pub trait Archive {
    fn is_message_in_archive(&self, message: &Message) -> bool;

    fn add_envelope_to_archive(&mut self, from: &str, envelope: &Envelope);
}

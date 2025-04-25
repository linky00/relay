use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{DateTime, Timelike, Utc};
use thiserror::Error;

use crate::{
    crypto::{PublicKey, SecretKey, get_canon_json_bytes},
    message::{Certificate, Envelope, Message, MessageContents},
    payload::TrustedPayload,
};

const DEFAULT_INITIAL_TTL: u8 = 8;
const DEFAULT_MAX_FORWARDING_TTL: u8 = 8;
const HOUR_IN_SECONDS: u64 = 60 * 60;

#[derive(Error, Debug)]
pub enum MailroomError<E> {
    #[error("already received payload from this key")]
    AlreadyReceivedFromKey,
    #[error("{0}")]
    ArchiveFailure(E),
}

pub struct Mailroom<L: GetNextLine, A: Archive<Error = E>, E> {
    line_generator: L,
    archive: A,
    secret_key: SecretKey,
    flatten_time: fn(DateTime<Utc>) -> DateTime<Utc>,
    interval: Duration,
    new_messages: HashSet<Message>,
    forwarding_received_this_hour: HashMap<PublicKey, Vec<Envelope>>,
    forwarding_received_last_hour: HashMap<PublicKey, Vec<Envelope>>,
    pub current_message: Option<Message>,
    last_seen_time: Option<DateTime<Utc>>,
}

impl<L: GetNextLine, A: Archive<Error = E>, E> Mailroom<L, A, E> {
    pub fn new(line_generator: L, archive: A, secret_key: SecretKey) -> Self {
        let flatten_time = |datetime: DateTime<Utc>| {
            datetime
                .with_minute(0)
                .expect("should be able to set any utc time to minute 0")
                .with_second(0)
                .expect("should be able to set any utc time to second 0")
                .with_nanosecond(0)
                .expect("should be able to set any utc time to nanosecond 0")
        };

        let mut mailroom = Mailroom {
            line_generator,
            archive,
            secret_key,
            flatten_time,
            interval: Duration::from_secs(HOUR_IN_SECONDS),
            new_messages: HashSet::new(),
            forwarding_received_this_hour: HashMap::new(),
            forwarding_received_last_hour: HashMap::new(),
            current_message: None,
            last_seen_time: None,
        };

        mailroom.set_new_message();

        mailroom
    }

    #[cfg(feature = "chrono")]
    pub fn new_with_custom_time(
        line_generator: L,
        archive: A,
        secret_key: SecretKey,
        flatten_time: fn(DateTime<Utc>) -> DateTime<Utc>,
        interval: Duration,
    ) -> Mailroom<L, A, E> {
        let mut mailroom = Self::new(line_generator, archive, secret_key);

        mailroom.flatten_time = flatten_time;
        mailroom.interval = interval;

        mailroom
    }

    pub async fn receive_payload(
        &mut self,
        payload: &TrustedPayload,
    ) -> Result<(), MailroomError<E>> {
        self.receive_payload_internal(payload, Utc::now()).await
    }

    #[cfg(feature = "chrono")]
    pub async fn receive_payload_at_time(
        &mut self,
        payload: &TrustedPayload,
        now: DateTime<Utc>,
    ) -> Result<(), MailroomError<E>> {
        self.receive_payload_internal(payload, now).await
    }

    async fn receive_payload_internal(
        &mut self,
        payload: &TrustedPayload,
        now: DateTime<Utc>,
    ) -> Result<(), MailroomError<E>> {
        self.handle_time(now);

        if self
            .forwarding_received_this_hour
            .contains_key(&payload.public_key)
        {
            return Err(MailroomError::AlreadyReceivedFromKey);
        }

        let mut forwarding_from_this_key = vec![];

        for envelope in &payload.envelopes {
            if self.new_messages.contains(&envelope.message) {
                forwarding_from_this_key.push(envelope.clone());
            } else if !self
                .archive
                .is_message_in_archive(&envelope.message)
                .await
                .map_err(|e| MailroomError::ArchiveFailure(e))?
            {
                self.new_messages.insert(envelope.message.clone());
                forwarding_from_this_key.push(envelope.clone());
            }

            self.archive
                .add_envelope_to_archive(&payload.certificate.key, envelope)
                .await
                .map_err(|e| MailroomError::ArchiveFailure(e))?;
        }

        self.forwarding_received_this_hour
            .insert(payload.public_key, forwarding_from_this_key);

        Ok(())
    }

    pub async fn get_outgoing(
        &mut self,
        sending_to: &PublicKey,
        ttl_config: TTLConfig,
    ) -> Result<OutgoingEnvelopes, MailroomError<E>> {
        self.get_outgoing_internal(sending_to, ttl_config, Utc::now())
            .await
    }

    #[cfg(feature = "chrono")]
    pub async fn get_outgoing_at_time(
        &mut self,
        sending_to: &PublicKey,
        ttl_config: TTLConfig,
        now: DateTime<Utc>,
    ) -> Result<OutgoingEnvelopes, MailroomError<E>> {
        self.get_outgoing_internal(sending_to, ttl_config, now)
            .await
    }

    async fn get_outgoing_internal(
        &mut self,
        sending_to: &PublicKey,
        ttl_config: TTLConfig,
        now: DateTime<Utc>,
    ) -> Result<OutgoingEnvelopes, MailroomError<E>> {
        self.handle_time(now);

        let mut sending_envelopes: Vec<Envelope> = self
            .forwarding_received_last_hour
            .iter()
            .filter(|(from_key, _)| *from_key != sending_to)
            .flat_map(|(_, envelopes)| envelopes.iter().cloned())
            .filter_map(|mut envelope| {
                envelope.ttl = ttl_config.max_forwarding_ttl.min(envelope.ttl - 1);
                if envelope.ttl > 0 {
                    Some(envelope)
                } else {
                    None
                }
            })
            .collect();

        if let Some(current_message) = &self.current_message {
            let envelope = Envelope {
                forwarded: vec![],
                ttl: ttl_config.initial_ttl,
                message: current_message.clone(),
            };

            self.archive
                .add_envelope_to_archive(&envelope.message.certificate.key, &envelope)
                .await
                .map_err(|e| MailroomError::ArchiveFailure(e))?;

            sending_envelopes.push(envelope);
        }

        Ok(OutgoingEnvelopes {
            envelopes: sending_envelopes,
            secret_key: self.secret_key.clone(),
        })
    }

    fn handle_time(&mut self, now: DateTime<Utc>) {
        if let Some(last_seen_time) = self.last_seen_time {
            let now_flattened = (self.flatten_time)(now);
            let last_seen_flattened = (self.flatten_time)(last_seen_time);

            if now_flattened != last_seen_flattened {
                self.forwarding_received_last_hour =
                    if now_flattened == last_seen_flattened + self.interval {
                        self.forwarding_received_this_hour.clone()
                    } else {
                        HashMap::new()
                    };
                self.forwarding_received_this_hour = HashMap::new();
                self.new_messages = HashSet::new();
                self.set_new_message();
            }
        }

        self.last_seen_time = Some(now);
    }

    fn set_new_message(&mut self) {
        self.current_message = if let Some(next_line) = self.line_generator.get_next_line() {
            let contents = MessageContents {
                uuid: uuid::Uuid::new_v4().hyphenated().to_string(),
                author: next_line.author.clone(),
                line: next_line.line.into(),
            };

            let contents_json = serde_json::to_string(&contents)
                .expect("should be able to serialize any message contents to json");

            let contents_bytes = get_canon_json_bytes(&contents_json)
                .expect("should be able to get canon bytes for any json string");

            let signature = self.secret_key.clone().sign(&contents_bytes);

            Some(Message {
                certificate: Certificate {
                    key: self.secret_key.public_key().to_string(),
                    signature,
                },
                contents,
            })
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct OutgoingEnvelopes {
    pub envelopes: Vec<Envelope>,
    pub(crate) secret_key: SecretKey,
}

#[derive(Clone, Copy)]
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

#[derive(Clone)]
pub struct NextLine {
    pub line: String,
    pub author: String,
}

pub trait GetNextLine {
    fn get_next_line(&mut self) -> Option<NextLine>;
}

#[trait_variant::make(Archive: Send)]
pub trait ArchiveLocal {
    type Error;

    async fn is_message_in_archive(&self, message: &Message) -> Result<bool, Self::Error>;

    async fn add_envelope_to_archive(
        &mut self,
        from: &str,
        envelope: &Envelope,
    ) -> Result<(), Self::Error>;
}

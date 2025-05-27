use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use relay_core::{
    crypto::{PublicKey, SecretKey},
    mailroom::{Archive, GetNextLine, Mailroom, MailroomError, NextLine, OutgoingConfig},
    message::{Envelope, Message},
    payload::{UntrustedPayload, UntrustedPayloadError},
};

#[derive(Debug)]
pub enum MockReceivePayloadError {
    ReadPayload(UntrustedPayloadError),
    TrustPayload(UntrustedPayloadError),
    ReceiveInMailroom(MailroomError<()>),
}

pub struct MockRelay {
    pub public_key: PublicKey,
    mailroom: Mailroom<MockLineGenerator, MockArchive, ()>,
    trusted_keys: HashSet<PublicKey>,
    #[allow(dead_code)]
    envelopes: Arc<Mutex<Vec<Envelope>>>,
    messages: Arc<Mutex<HashSet<Message>>>,
    send_on_minute: u32,
    last_message: Arc<Mutex<Option<String>>>,
    message_count: Arc<Mutex<u32>>,
}

impl MockRelay {
    pub fn new(name: &str, send_on_minute: u32) -> Self {
        let secret_key = SecretKey::generate();

        let envelopes = Arc::new(Mutex::new(vec![]));
        let messages = Arc::new(Mutex::new(HashSet::new()));
        let last_message = Arc::new(Mutex::new(None));
        let message_count = Arc::new(Mutex::new(0));

        MockRelay {
            public_key: secret_key.public_key(),
            mailroom: Mailroom::new(
                MockLineGenerator {
                    name: name.to_owned(),
                    last_message: Arc::clone(&last_message),
                    message_count: Arc::clone(&message_count),
                },
                MockArchive {
                    envelopes: Arc::clone(&envelopes),
                    messages: Arc::clone(&messages),
                },
                secret_key,
            )
            .unwrap(),
            trusted_keys: HashSet::new(),
            envelopes,
            messages,
            send_on_minute,
            last_message,
            message_count,
        }
    }

    pub fn message_count(&self) -> u32 {
        *self.message_count.lock()
    }

    pub fn add_trusted_key(&mut self, key: PublicKey) {
        self.trusted_keys.insert(key);
    }

    pub async fn receive_payload(
        &mut self,
        payload: &str,
        at: DateTime<Utc>,
    ) -> Result<(), MockReceivePayloadError> {
        let unverified_payload =
            UntrustedPayload::from_json(payload).map_err(MockReceivePayloadError::ReadPayload)?;
        let verified_payload = unverified_payload
            .try_trust(self.trusted_keys.clone())
            .map_err(MockReceivePayloadError::TrustPayload)?;
        self.mailroom
            .receive_payload_at_time(&verified_payload, at)
            .await
            .map_err(MockReceivePayloadError::ReceiveInMailroom)?;
        Ok(())
    }

    pub async fn create_payload(&mut self, for_key: PublicKey, at: DateTime<Utc>) -> String {
        let outgoing_envelopes = self
            .mailroom
            .get_outgoing_at_time(
                &for_key,
                OutgoingConfig::new(Some(self.send_on_minute), None, None),
                at,
            )
            .await
            .unwrap();
        outgoing_envelopes.create_payload()
    }

    pub fn has_message_with_line(&self, line: &str) -> bool {
        self.messages
            .lock()
            .iter()
            .any(|message| message.contents.line == line)
    }

    pub fn has_message_from(&self, from_key: PublicKey) -> bool {
        self.messages
            .lock()
            .iter()
            .any(|message| *message.certificate.key == from_key.to_string())
    }

    pub fn has_forwarded_from(&self, from_key: PublicKey) -> bool {
        self.envelopes.lock().iter().any(|envelope| {
            envelope
                .forwarded
                .iter()
                .any(|forwarded| *forwarded == from_key.to_string())
        })
    }

    pub fn current_line(&self) -> Option<String> {
        self.last_message.lock().clone()
    }
}

struct MockLineGenerator {
    name: String,
    last_message: Arc<Mutex<Option<String>>>,
    message_count: Arc<Mutex<u32>>,
}

impl GetNextLine for MockLineGenerator {
    fn get_next_line(&mut self) -> Option<NextLine> {
        let line = format!("{}: {}", self.name, uuid::Uuid::new_v4().hyphenated());
        *self.last_message.lock() = Some(line.clone());
        *self.message_count.lock() += 1;
        Some(NextLine {
            line,
            author: self.name.clone(),
        })
    }
}

struct MockArchive {
    envelopes: Arc<Mutex<Vec<Envelope>>>,
    messages: Arc<Mutex<HashSet<Message>>>,
}

impl Archive for MockArchive {
    type Error = ();

    async fn add_envelope_to_archive(&mut self, _: &str, envelope: &Envelope) -> Result<(), ()> {
        self.envelopes.lock().push(envelope.clone());
        self.messages.lock().insert(envelope.message.clone());
        Ok(())
    }

    async fn is_message_in_archive(&self, message: &Message) -> Result<bool, ()> {
        Ok(self.messages.lock().contains(message))
    }
}

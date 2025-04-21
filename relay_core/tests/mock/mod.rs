use std::{cell::RefCell, collections::HashSet, rc::Rc};

use chrono::{DateTime, Utc};
use relay_core::{
    crypto::{PublicKey, SecretKey},
    mailroom::{Archive, GetNextLine, Line, Mailroom, ReceivePayloadError, TTLConfig},
    message::{Envelope, Message},
    payload::{UntrustedPayload, UntrustedPayloadError},
};

#[derive(Debug)]
pub enum MockReceivePayloadError {
    CannotReadPayload(UntrustedPayloadError),
    CannotTrustPayload(UntrustedPayloadError),
    CannotReceiveInMailroom(ReceivePayloadError),
}

pub struct MockRelay {
    pub public_key: PublicKey,
    mailroom: Mailroom<MockLineGenerator, MockArchive>,
    trusted_keys: HashSet<PublicKey>,
    #[allow(dead_code)]
    envelopes: Rc<RefCell<Vec<Envelope>>>,
    messages: Rc<RefCell<HashSet<Message>>>,
}

impl MockRelay {
    pub fn new(name: &str) -> Self {
        let secret_key = SecretKey::generate();

        let envelopes = Rc::new(RefCell::new(vec![]));
        let messages = Rc::new(RefCell::new(HashSet::new()));

        MockRelay {
            public_key: secret_key.public_key(),
            mailroom: Mailroom::new(
                MockLineGenerator {
                    name: name.to_owned(),
                },
                MockArchive {
                    envelopes: Rc::clone(&envelopes),
                    messages: Rc::clone(&messages),
                },
                secret_key,
            ),
            trusted_keys: HashSet::new(),
            envelopes,
            messages,
        }
    }

    pub fn add_trusted_key(&mut self, key: PublicKey) {
        self.trusted_keys.insert(key);
    }

    pub fn receive_payload(
        &mut self,
        payload: &str,
        at: DateTime<Utc>,
    ) -> Result<(), MockReceivePayloadError> {
        let unverified_payload = UntrustedPayload::from_json(payload)
            .map_err(|e| MockReceivePayloadError::CannotReadPayload(e))?;
        let verified_payload = unverified_payload
            .try_trust(self.trusted_keys.clone())
            .map_err(|e| MockReceivePayloadError::CannotTrustPayload(e))?;
        self.mailroom
            .receive_payload_at_time(verified_payload, at)
            .map_err(|e| MockReceivePayloadError::CannotReceiveInMailroom(e))?;
        Ok(())
    }

    pub fn create_payload(&mut self, for_key: PublicKey, at: DateTime<Utc>) -> String {
        let outgoing_envelopes =
            self.mailroom
                .get_outgoing_at_time(&for_key, TTLConfig::default(), at);
        outgoing_envelopes.create_payload()
    }

    pub fn messages(&self) -> HashSet<Message> {
        self.messages.borrow().clone()
    }

    pub fn has_message_with_line(&self, line: &str) -> bool {
        self.messages
            .borrow()
            .iter()
            .any(|message| message.contents.line == line)
    }

    pub fn current_line(&self) -> Option<String> {
        self.mailroom
            .current_message
            .clone()
            .map(|message| message.contents.line)
    }
}

struct MockLineGenerator {
    name: String,
}

impl GetNextLine for MockLineGenerator {
    fn get_next_line(&mut self) -> Option<Line> {
        Some(Line {
            text: format!("{}: {}", self.name, uuid::Uuid::new_v4().hyphenated()),
            author: self.name.clone(),
        })
    }
}

struct MockArchive {
    envelopes: Rc<RefCell<Vec<Envelope>>>,
    messages: Rc<RefCell<HashSet<Message>>>,
}

impl Archive for MockArchive {
    fn add_envelope_to_archive(&mut self, _: &str, envelope: &Envelope) {
        self.envelopes.borrow_mut().push(envelope.clone());
        self.messages.borrow_mut().insert(envelope.message.clone());
    }

    fn is_message_in_archive(&self, message: &Message) -> bool {
        self.messages.borrow().contains(message)
    }
}

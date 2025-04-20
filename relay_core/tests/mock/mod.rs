use std::{cell::RefCell, collections::HashSet, rc::Rc};

use chrono::{DateTime, Utc};
use relay_core::{
    crypto::{PublicKey, SecretKey},
    mailroom::{Archive, Mailroom, OutgoingConfig, TTLConfig},
    message::{Envelope, Message},
    payload::UntrustedPayload,
};

pub struct MockRelay {
    pub public_key: PublicKey,
    mailroom: Mailroom<MockArchive>,
    outgoing_config: OutgoingConfig,
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
            mailroom: Mailroom::new(MockArchive {
                envelopes: Rc::clone(&envelopes),
                messages: Rc::clone(&messages),
            }),
            outgoing_config: OutgoingConfig::new(name, secret_key, TTLConfig::default()),
            trusted_keys: HashSet::new(),
            envelopes,
            messages,
        }
    }

    pub fn add_trusted_key(&mut self, key: PublicKey) {
        self.trusted_keys.insert(key);
    }

    pub fn receive_payload(&mut self, payload: &str, now: DateTime<Utc>) {
        let unverified_payload = UntrustedPayload::from_json(payload).unwrap();
        let verified_payload = unverified_payload
            .try_trust(self.trusted_keys.clone())
            .unwrap();
        self.mailroom
            .receive_payload_at_time(verified_payload, now)
            .unwrap();
    }

    pub fn create_payload(&mut self, for_key: PublicKey, line: &str, now: DateTime<Utc>) -> String {
        let outgoing_envelopes =
            self.mailroom
                .get_outgoing_at_time(&for_key, Some(line), &self.outgoing_config, now);
        outgoing_envelopes.create_payload()
    }

    pub fn has_message_with_line(&self, line: &str) -> bool {
        self.messages
            .borrow()
            .iter()
            .any(|message| message.contents.line == line)
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

use std::collections::HashSet;

use relay_core::{
    mailroom::Archive,
    message::{Envelope, Message},
};

pub struct MockArchive {
    pub envelopes: Vec<Envelope>,
    pub messages: HashSet<Message>,
}

impl MockArchive {
    pub fn new() -> Self {
        MockArchive {
            envelopes: vec![],
            messages: HashSet::new(),
        }
    }
}

impl Archive for MockArchive {
    fn add_envelope_to_archive(&mut self, _: &relay_core::message::RelayID, envelope: &Envelope) {
        self.envelopes.push(envelope.clone());
        self.messages.insert(envelope.message.clone());
    }

    fn is_message_in_archive(&self, message: &Message) -> bool {
        self.messages.contains(message)
    }
}

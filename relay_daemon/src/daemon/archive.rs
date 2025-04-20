use std::collections::HashSet;

use relay_core::{
    mailroom::Archive,
    message::{Envelope, Message},
};

pub(crate) struct MockArchive {
    envelopes: Vec<Envelope>,
    messages: HashSet<Message>,
}

impl MockArchive {
    pub(crate) fn new() -> Self {
        Self {
            envelopes: vec![],
            messages: HashSet::new(),
        }
    }
}

impl Archive for MockArchive {
    fn add_envelope_to_archive(&mut self, _: &str, envelope: &Envelope) {
        self.envelopes.push(envelope.clone());
        self.messages.insert(envelope.message.clone());
    }

    fn is_message_in_archive(&self, message: &Message) -> bool {
        self.messages.contains(message)
    }
}

use std::collections::HashSet;

use relay_core::{
    crypto::PublicKey,
    mailroom::{Archive, Mailroom},
};

use crate::config::ReadConfig;

pub struct RelayServer<R: ReadConfig> {
    mailroom: Mailroom<DBArchive>,
    config: R,
}

impl<R: ReadConfig> RelayServer<R> {}

pub struct DBArchive;

impl Archive for DBArchive {
    fn add_envelope_to_archive(
        &mut self,
        from: &relay_core::message::RelayID,
        envelope: &relay_core::message::Envelope,
    ) {
        todo!()
    }

    fn is_message_in_archive(&self, message: &relay_core::message::Message) -> bool {
        todo!()
    }
}

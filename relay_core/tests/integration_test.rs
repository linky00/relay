use common::MockArchive;
use relay_core::{
    crypto::SecretKey,
    mailroom::{Mailroom, MailroomConfig, TTLConfig},
};

mod common;

#[test]
fn create_payload() {
    let secret_key = SecretKey::generate();
    let config = MailroomConfig::new("ada's relay", secret_key, TTLConfig::default());
    let mock_archive = MockArchive::new();
    let mut mailroom = Mailroom::new(config, mock_archive);

    let sending_to = SecretKey::generate().public_key();
    let outgoing_envelopes = mailroom.get_outgoing(&sending_to, Some("poetry!"));
    outgoing_envelopes.create_payload().unwrap();
}

use common::MockArchive;
use relay_core::{
    crypto::SecretKey,
    mailroom::{Mailroom, OutgoingConfig, TTLConfig},
};

mod common;

#[test]
fn create_payload() {
    let secret_key = SecretKey::generate();
    let mock_archive = MockArchive::new();
    let mut mailroom = Mailroom::new(mock_archive);

    let sending_to = SecretKey::generate().public_key();
    let outgoing_config = OutgoingConfig::new("ada's relay", secret_key, TTLConfig::default());
    let outgoing_envelopes = mailroom.get_outgoing(&sending_to, Some("poetry!"), &outgoing_config);
    outgoing_envelopes.create_payload().unwrap();
}

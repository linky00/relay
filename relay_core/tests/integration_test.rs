use std::time::Duration;

use chrono::{DateTime, Utc};
use itertools::Itertools;
use mock::{MockReceivePayloadError, MockRelay};
use relay_core::{
    crypto::SecretKey, mailroom::ReceivePayloadError, payload::UntrustedPayloadError,
};

mod mock;

fn mutually_trust(relay_a: &mut MockRelay, relay_b: &mut MockRelay) {
    relay_a.add_trusted_key(relay_b.public_key);
    relay_b.add_trusted_key(relay_a.public_key);
}

fn send_payload(
    from_relay: &mut MockRelay,
    to_relay: &mut MockRelay,
    at: DateTime<Utc>,
) -> Result<(), MockReceivePayloadError> {
    let payload = from_relay.create_payload(to_relay.public_key, at);
    to_relay.receive_payload(&payload, at)
}

#[test]
fn relay_exchange() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    mutually_trust(&mut relay_a, &mut relay_b);

    let now = Utc::now();

    send_payload(&mut relay_a, &mut relay_b, now).unwrap();
    assert!(relay_b.has_message_with_line(&relay_a.current_line().unwrap()));

    send_payload(&mut relay_b, &mut relay_a, now).unwrap();
    assert!(relay_a.has_message_with_line(&relay_b.current_line().unwrap()));
}

#[test]
fn reject_malformed() {
    let mut relay_a = MockRelay::new("a");

    assert!(matches!(
        relay_a.receive_payload("{\"fact\": \"this json is nonsense\"}", Utc::now()),
        Err(MockReceivePayloadError::CannotReadPayload(
            UntrustedPayloadError::CannotParseJson
        ))
    ))
}

#[test]
fn reject_untrusted() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, Utc::now()),
        Err(MockReceivePayloadError::CannotTrustPayload(
            UntrustedPayloadError::PublicKeyNotTrusted
        ))
    ));
}

#[test]
fn reject_already_received_this_hour() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    mutually_trust(&mut relay_a, &mut relay_b);

    let now = Utc::now();

    send_payload(&mut relay_a, &mut relay_b, now).unwrap();
    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, now),
        Err(MockReceivePayloadError::CannotReceiveInMailroom(
            ReceivePayloadError::AlreadyReceivedFromKey
        ))
    ));
}

#[test]
fn send_different_line_every_hour() {
    let mut relay_a = MockRelay::new("a");

    let mut time = Utc::now();
    let mut lines = vec![];

    for _ in 0..10 {
        relay_a.create_payload(SecretKey::generate().public_key(), time);
        lines.push(relay_a.current_line());
        time += Duration::from_secs(3600);
    }

    assert!(lines.iter().all_unique())
}

#[test]
fn send_same_line_in_same_hour() {
    let mut relay_a = MockRelay::new("a");

    let now = Utc::now();

    relay_a.create_payload(SecretKey::generate().public_key(), now);
    let first_line = relay_a.current_line();

    relay_a.create_payload(SecretKey::generate().public_key(), now);
    let second_line = relay_a.current_line();

    assert_eq!(first_line, second_line);
}

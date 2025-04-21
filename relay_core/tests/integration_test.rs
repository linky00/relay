use chrono::{DateTime, Utc};
use mock::{MockReceivePayloadError, MockRelay};
use relay_core::{mailroom::ReceivePayloadError, payload::UntrustedPayloadError};

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

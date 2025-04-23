use std::time::Duration;

use chrono::{DateTime, Utc};
use itertools::Itertools;
use mock::{MockReceivePayloadError, MockRelay};
use relay_core::{crypto::SecretKey, mailroom::MailroomError, payload::UntrustedPayloadError};

mod mock;

fn mutually_trust(relay_a: &mut MockRelay, relay_b: &mut MockRelay) {
    relay_a.add_trusted_key(relay_b.public_key);
    relay_b.add_trusted_key(relay_a.public_key);
}

async fn send_payload(
    from_relay: &mut MockRelay,
    to_relay: &mut MockRelay,
    at: DateTime<Utc>,
) -> Result<(), MockReceivePayloadError> {
    let payload = from_relay.create_payload(to_relay.public_key, at).await;
    to_relay.receive_payload(&payload, at).await
}

async fn exchange_payloads(
    relay_a: &mut MockRelay,
    relay_b: &mut MockRelay,
    at: DateTime<Utc>,
) -> Result<(), MockReceivePayloadError> {
    send_payload(relay_a, relay_b, at).await?;
    send_payload(relay_b, relay_a, at).await?;
    Ok(())
}

#[tokio::test]
async fn reject_malformed() {
    let mut relay_a = MockRelay::new("a");

    assert!(matches!(
        relay_a
            .receive_payload("{\"fact\": \"this json is nonsense\"}", Utc::now())
            .await,
        Err(MockReceivePayloadError::CannotReadPayload(
            UntrustedPayloadError::CannotParseJson
        ))
    ))
}

#[tokio::test]
async fn reject_untrusted() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, Utc::now()).await,
        Err(MockReceivePayloadError::CannotTrustPayload(
            UntrustedPayloadError::PublicKeyNotTrusted
        ))
    ));
}

#[tokio::test]
async fn reject_already_received_this_hour() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    mutually_trust(&mut relay_a, &mut relay_b);

    let now = Utc::now();

    send_payload(&mut relay_a, &mut relay_b, now).await.unwrap();
    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, now).await,
        Err(MockReceivePayloadError::CannotReceiveInMailroom(
            MailroomError::AlreadyReceivedFromKey
        ))
    ));
}

#[tokio::test]
async fn send_different_line_every_hour() {
    let mut relay_a = MockRelay::new("a");

    let mut time = Utc::now();
    let mut lines = vec![];

    for _ in 0..10 {
        relay_a
            .create_payload(SecretKey::generate().public_key(), time)
            .await;
        lines.push(relay_a.current_line());
        time += Duration::from_secs(3600);
    }

    assert!(lines.iter().all_unique())
}

#[tokio::test]
async fn send_same_line_in_same_hour() {
    let mut relay_a = MockRelay::new("a");

    let now = Utc::now();

    relay_a
        .create_payload(SecretKey::generate().public_key(), now)
        .await;
    let first_line = relay_a.current_line();

    relay_a
        .create_payload(SecretKey::generate().public_key(), now)
        .await;
    let second_line = relay_a.current_line();

    assert_eq!(first_line, second_line);
}

#[tokio::test]
async fn relay_exchange() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");

    mutually_trust(&mut relay_a, &mut relay_b);

    exchange_payloads(&mut relay_a, &mut relay_b, Utc::now())
        .await
        .unwrap();

    assert!(relay_b.has_message_with_line(&relay_a.current_line().unwrap()));
    assert!(relay_a.has_message_with_line(&relay_b.current_line().unwrap()));
}

#[tokio::test]
async fn relay_chain() {
    let mut relay_a = MockRelay::new("a");
    let mut relay_b = MockRelay::new("b");
    let mut relay_c = MockRelay::new("c");

    mutually_trust(&mut relay_a, &mut relay_b);
    mutually_trust(&mut relay_b, &mut relay_c);

    let now = Utc::now();
    exchange_payloads(&mut relay_a, &mut relay_b, now)
        .await
        .unwrap();
    exchange_payloads(&mut relay_b, &mut relay_c, now)
        .await
        .unwrap();

    let relay_a_line = relay_a.current_line().unwrap();

    assert!(relay_a.has_message_with_line(&relay_a_line));
    assert!(relay_b.has_message_with_line(&relay_a_line));
    assert!(!relay_c.has_message_with_line(&relay_a_line));

    let an_hour_later = now + Duration::from_secs(3600);
    exchange_payloads(&mut relay_a, &mut relay_b, an_hour_later)
        .await
        .unwrap();
    exchange_payloads(&mut relay_b, &mut relay_c, an_hour_later)
        .await
        .unwrap();

    assert!(relay_a.has_message_with_line(&relay_a_line));
    assert!(relay_b.has_message_with_line(&relay_a_line));
    assert!(relay_c.has_message_with_line(&relay_a_line));
}

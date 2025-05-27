use std::time::Duration;

use chrono::{DateTime, Timelike, Utc};
use itertools::Itertools;
use mock::{MockReceivePayloadError, MockRelay};
use relay_core::{
    crypto::SecretKey,
    mailroom::{DEFAULT_INITIAL_TTL, MailroomError},
    payload::UntrustedPayloadError,
};

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
    let mut relay_a = MockRelay::new("a", 0);

    assert!(matches!(
        relay_a
            .receive_payload("{\"fact\": \"this json is nonsense\"}", Utc::now())
            .await,
        Err(MockReceivePayloadError::ReadPayload(
            UntrustedPayloadError::CannotParseJson
        ))
    ))
}

#[tokio::test]
async fn reject_untrusted() {
    let mut relay_a = MockRelay::new("a", 0);
    let mut relay_b = MockRelay::new("b", 0);

    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, Utc::now()).await,
        Err(MockReceivePayloadError::TrustPayload(
            UntrustedPayloadError::PublicKeyNotTrusted
        ))
    ));
}

#[tokio::test]
async fn reject_already_received_this_minute() {
    let mut relay_a = MockRelay::new("a", 0);
    let mut relay_b = MockRelay::new("b", 0);

    mutually_trust(&mut relay_a, &mut relay_b);

    let now = Utc::now();

    send_payload(&mut relay_a, &mut relay_b, now).await.unwrap();
    assert!(matches!(
        send_payload(&mut relay_a, &mut relay_b, now).await,
        Err(MockReceivePayloadError::ReceiveInMailroom(
            MailroomError::AlreadyReceivedFromKey
        ))
    ));
}

#[tokio::test]
async fn send_different_line_every_hour() {
    let mut time = Utc::now();
    let mut relay_a = MockRelay::new("a", time.minute());

    let mut lines = vec![];

    for _ in 0..10 {
        relay_a
            .create_payload(SecretKey::generate().public_key(), time)
            .await;
        lines.push(relay_a.current_line());
        time += Duration::from_secs(3600);
    }

    assert!(lines.iter().all(|line| line.is_some()));
    assert!(lines.iter().all_unique());
}

#[tokio::test]
async fn send_same_line_in_same_hour() {
    let now = Utc::now();
    let mut relay_a = MockRelay::new("a", now.minute());

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
    let now = Utc::now();
    let mut relay_a = MockRelay::new("a", now.minute());
    let mut relay_b = MockRelay::new("b", now.minute());

    mutually_trust(&mut relay_a, &mut relay_b);

    exchange_payloads(&mut relay_a, &mut relay_b, now)
        .await
        .unwrap();

    assert!(relay_b.has_message_with_line(&relay_a.current_line().unwrap()));
    assert!(relay_a.has_message_with_line(&relay_b.current_line().unwrap()));
}

#[tokio::test]
async fn relay_chain() {
    let now = Utc::now();

    let mut relay_a = MockRelay::new("a", now.minute());
    let mut relay_b = MockRelay::new("b", 0);
    let mut relay_c = MockRelay::new("c", 0);

    mutually_trust(&mut relay_a, &mut relay_b);
    mutually_trust(&mut relay_b, &mut relay_c);

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
    assert!(!relay_c.has_forwarded_from(relay_b.public_key));

    let a_minute_later = now + Duration::from_secs(60);

    exchange_payloads(&mut relay_a, &mut relay_b, a_minute_later)
        .await
        .unwrap();
    exchange_payloads(&mut relay_b, &mut relay_c, a_minute_later)
        .await
        .unwrap();

    assert!(relay_a.has_message_with_line(&relay_a_line));
    assert!(relay_b.has_message_with_line(&relay_a_line));
    assert!(relay_c.has_message_with_line(&relay_a_line));
    assert!(relay_c.has_forwarded_from(relay_b.public_key));
}

#[tokio::test]
async fn ttl_exhaustion() {
    let mut current_time = Utc::now();
    let mut current_relay = MockRelay::new("origin", current_time.minute());
    let origin_key = current_relay.public_key;

    for i in 0..DEFAULT_INITIAL_TTL {
        let mut next_relay = MockRelay::new(&i.to_string(), 0);
        mutually_trust(&mut current_relay, &mut next_relay);
        exchange_payloads(&mut current_relay, &mut next_relay, current_time)
            .await
            .unwrap();
        assert!(next_relay.has_message_from(origin_key));
        current_relay = next_relay;
        current_time += Duration::from_secs(60);
    }

    let mut final_relay = MockRelay::new("last", 0);
    mutually_trust(&mut current_relay, &mut final_relay);
    exchange_payloads(&mut current_relay, &mut final_relay, current_time)
        .await
        .unwrap();
    assert!(!final_relay.has_message_from(origin_key));
}

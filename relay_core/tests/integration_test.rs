use chrono::Utc;
use mock::MockRelay;

mod mock;

const RELAY_A_NAME: &str = "Relay A";
const RELAY_B_NAME: &str = "Relay B";

fn create_relay_a() -> MockRelay {
    MockRelay::new(RELAY_A_NAME)
}

fn create_relay_b() -> MockRelay {
    MockRelay::new(RELAY_B_NAME)
}

#[test]
fn relays_talking() {
    const A_LINE: &str = "a's line";
    const B_LINE: &str = "b's line";

    let mut relay_a = create_relay_a();
    let mut relay_b = create_relay_b();

    relay_a.add_trusted_key(relay_b.public_key);
    relay_b.add_trusted_key(relay_a.public_key);

    let now = Utc::now();

    let relay_a_payload = relay_a.create_payload(relay_b.public_key, A_LINE, now);
    relay_b.receive_payload(&relay_a_payload, now);
    assert!(relay_b.has_message_with_line(A_LINE));

    let relay_b_payload = relay_b.create_payload(relay_a.public_key, B_LINE, now);
    relay_a.receive_payload(&relay_b_payload, now);
    assert!(relay_a.has_message_with_line(B_LINE));
}

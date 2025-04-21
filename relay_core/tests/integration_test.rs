use chrono::Utc;
use mock::MockRelay;

mod mock;

const RELAY_A_NAME: &str = "Relay A";
const RELAY_A_LINE: &str = "relay a's line";
const RELAY_B_NAME: &str = "Relay B";
const RELAY_B_LINE: &str = "relay b's line";

fn create_relay_a() -> MockRelay {
    MockRelay::new(RELAY_A_NAME, RELAY_A_LINE)
}

fn create_relay_b() -> MockRelay {
    MockRelay::new(RELAY_B_NAME, RELAY_B_LINE)
}

#[test]
fn relays_talking() {
    let mut relay_a = create_relay_a();
    let mut relay_b = create_relay_b();

    relay_a.add_trusted_key(relay_b.public_key);
    relay_b.add_trusted_key(relay_a.public_key);

    let now = Utc::now();

    let relay_a_payload = relay_a.create_payload(relay_b.public_key, now);
    relay_b.receive_payload(&relay_a_payload, now);
    assert!(relay_b.has_message_with_line(RELAY_A_LINE));

    let relay_b_payload = relay_b.create_payload(relay_a.public_key, now);
    relay_a.receive_payload(&relay_b_payload, now);
    assert!(relay_a.has_message_with_line(RELAY_B_LINE));
}

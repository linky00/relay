use bincode::Encode;
use chrono::{DateTime, Timelike, Utc};
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, Signature, SigningKey, VerifyingKey, ed25519::signature::SignerMut,
};
use rand::rngs::OsRng;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_TTL: u8 = 10;

pub struct Payload {
    verifying_key: VerifyingKey,
    signature: Signature,
    messages: Vec<Message>,
}

#[derive(Error, Debug)]
pub enum ReceivePayloadError {
    #[error("verifying key is not trusted by relay")]
    KeyNotTrusted,
    #[error("cannot encode message into bytes")]
    CannotEncodeMessage,
    #[error("cannot verify message with given verifying key and signature")]
    CannotVerifyMessage,
}

#[derive(Error, Debug)]
pub enum CreatePayloadError {
    #[error("cannot encode message into bytes")]
    CannotEncodeMessage,
}

#[derive(Error, Debug)]
pub enum PublicKeyError {
    #[error("cannot read key")]
    CannotReadKey,
}

pub struct Relay {
    signing_key: SigningKey,
    trusted_keys: HashSet<VerifyingKey>,
    message_handler: MessageHandler,
}

impl Relay {
    pub fn new<S, I>(name: S, poem: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let signing_key: SigningKey = SigningKey::generate(&mut OsRng);
        let message_handler = MessageHandler::new(name, poem);

        Self {
            signing_key,
            trusted_keys: HashSet::new(),
            message_handler,
        }
    }

    pub fn receive_payload(&mut self, payload: &Payload) -> Result<(), ReceivePayloadError> {
        if !self.trusted_keys.contains(&payload.verifying_key) {
            return Err(ReceivePayloadError::KeyNotTrusted);
        }

        let message_bytes = get_messages_bytes(&payload.messages)
            .map_err(|_| ReceivePayloadError::CannotEncodeMessage)?;

        match payload
            .verifying_key
            .verify_strict(&message_bytes, &payload.signature)
        {
            Ok(_) => {
                self.message_handler.receive_messages(&payload.messages);
                Ok(())
            }
            Err(_) => Err(ReceivePayloadError::CannotVerifyMessage),
        }
    }

    pub fn create_payload(&mut self, now: DateTime<Utc>) -> Result<Payload, CreatePayloadError> {
        let messages = self.message_handler.get_messages(now);

        let message_bytes =
            get_messages_bytes(&messages).map_err(|_| CreatePayloadError::CannotEncodeMessage)?;

        let signature = self.signing_key.sign(&message_bytes);

        Ok(Payload {
            verifying_key: self.signing_key.verifying_key(),
            signature,
            messages,
        })
    }

    pub fn get_public_key(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn trust_public_key(
        &mut self,
        public_key: &[u8; PUBLIC_KEY_LENGTH],
    ) -> Result<(), PublicKeyError> {
        let verifying_key =
            VerifyingKey::from_bytes(public_key).map_err(|_| PublicKeyError::CannotReadKey)?;

        self.trusted_keys.insert(verifying_key);
        Ok(())
    }

    pub fn forget_public_key(
        &mut self,
        public_key: &[u8; PUBLIC_KEY_LENGTH],
    ) -> Result<(), PublicKeyError> {
        let verifying_key =
            VerifyingKey::from_bytes(public_key).map_err(|_| PublicKeyError::CannotReadKey)?;

        self.trusted_keys.remove(&verifying_key);
        Ok(())
    }
}

struct MessageHandler {
    name: String,
    poem: Vec<String>,
    next_line_idx: usize,
    outgoing_messages: Vec<Message>,
    last_generated_at: Option<DateTime<Utc>>,
    received_messages: HashMap<Uuid, Message>,
    archived_messages: HashMap<Uuid, Message>,
}

impl MessageHandler {
    pub fn new<S, I>(name: S, poem: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        Self {
            name: name.into(),
            poem: poem.into_iter().map(|s| s.as_ref().to_owned()).collect(),
            next_line_idx: 0,
            outgoing_messages: vec![],
            last_generated_at: None,
            received_messages: HashMap::new(),
            archived_messages: HashMap::new(),
        }
    }

    pub fn receive_messages(&mut self, messages: &Vec<Message>) {
        for received_message in messages {
            if self
                .archived_messages
                .keys()
                .any(|archived_uuid| *archived_uuid == received_message.uuid)
            {
                continue;
            }

            self.received_messages
                .entry(received_message.uuid)
                .or_insert(received_message.clone());
        }
    }

    pub fn get_messages(&mut self, now: DateTime<Utc>) -> Vec<Message> {
        if let Some(last_generated_at) = self.last_generated_at {
            if now.date_naive() != last_generated_at.date_naive()
                || now.hour() != last_generated_at.hour()
            {
                self.regenerate_outgoing_messages(now);
            }
        } else {
            self.regenerate_outgoing_messages(now);
        }

        self.outgoing_messages.clone()
    }

    fn regenerate_outgoing_messages(&mut self, now: DateTime<Utc>) {
        self.outgoing_messages = self.received_messages.values().cloned().collect();

        let new_message = Message {
            uuid: Uuid::new_v4(),
            line: self.poem[self.next_line_idx % self.poem.len()].clone(),
            author: self.name.clone(),
            ttl: INITIAL_TTL,
        };
        self.next_line_idx += 1;
        self.outgoing_messages.push(new_message);

        self.outgoing_messages.iter_mut().for_each(|message| {
            message.ttl -= 1;
        });
        self.outgoing_messages.retain(|message| message.ttl > 0);

        for (old_uuid, old_message) in self.received_messages.drain() {
            self.archived_messages.insert(old_uuid, old_message);
        }

        self.last_generated_at = Some(now);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Message {
    uuid: Uuid,
    line: String,
    author: String,
    ttl: u8,
}

fn get_messages_bytes(messages: &Vec<Message>) -> Result<Vec<u8>, bincode::error::EncodeError> {
    bincode::encode_to_vec(messages.clone(), bincode::config::standard())
}

impl Encode for Message {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        bincode::Encode::encode(&self.uuid.as_bytes(), encoder)?;
        bincode::Encode::encode(&self.line, encoder)?;
        bincode::Encode::encode(&self.author, encoder)?;
        bincode::Encode::encode(&self.ttl, encoder)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use chrono::Duration;

    use super::*;

    const RELAY_A_NAME: &str = "relay a";
    const RELAY_A_POEM: [&str; 3] = [
        "relay a's line one",
        "relay a's line two",
        "relay a's line three",
    ];
    const RELAY_B_NAME: &str = "relay b";
    const RELAY_B_POEM: [&str; 3] = [
        "relay b's line one",
        "relay b's line two",
        "relay b's line three",
    ];

    fn create_relay_a() -> Relay {
        Relay::new(RELAY_A_NAME, RELAY_A_POEM)
    }

    fn create_relay_b() -> Relay {
        Relay::new(RELAY_B_NAME, RELAY_B_POEM)
    }

    #[test]
    fn test_same_messages_at_same_time() -> Result<()> {
        let mut relay = create_relay_a();
        let now = Utc::now();

        assert_eq!(
            relay.create_payload(now)?.messages,
            relay.create_payload(now)?.messages
        );

        Ok(())
    }

    #[test]
    fn test_loop_through_poem() -> Result<()> {
        let mut relay = create_relay_a();

        for i in 0..10 {
            let current_time = Utc::now() + Duration::hours(i.try_into().unwrap());

            let payload = relay.create_payload(current_time)?;
            assert_eq!(payload.messages[0].author, RELAY_A_NAME);
            assert_eq!(
                payload.messages[0].line,
                RELAY_A_POEM[i % RELAY_A_POEM.len()]
            );
        }

        Ok(())
    }

    #[test]
    fn test_verify_keys() -> Result<()> {
        let mut relay_a = create_relay_a();
        let mut relay_b = create_relay_b();
        let now = Utc::now();

        let relay_a_public_key_bytes = relay_a.get_public_key();
        let relay_b_public_key_bytes = relay_b.get_public_key();

        let relay_a_payload = relay_a.create_payload(now)?;
        let relay_b_payload = relay_b.create_payload(now)?;

        assert!(relay_a.receive_payload(&relay_b_payload).is_err());
        assert!(relay_b.receive_payload(&relay_a_payload).is_err());

        relay_a.trust_public_key(&relay_b_public_key_bytes)?;
        relay_b.trust_public_key(&relay_a_public_key_bytes)?;

        relay_a.receive_payload(&relay_b_payload)?;
        relay_b.receive_payload(&relay_a_payload)?;

        Ok(())
    }

    #[test]
    fn test_relay_message() -> Result<()> {
        let mut relay_a = create_relay_a();
        let mut relay_b = create_relay_b();

        let now = Utc::now();

        relay_a.trust_public_key(&relay_b.get_public_key())?;
        relay_b.trust_public_key(&relay_a.get_public_key())?;

        let relay_a_payload = relay_a.create_payload(now)?;
        let relay_b_payload = relay_b.create_payload(now)?;

        relay_a.receive_payload(&relay_b_payload)?;
        relay_b.receive_payload(&relay_a_payload)?;

        assert!(
            !relay_a
                .create_payload(now)?
                .messages
                .iter()
                .any(|message| message.line == RELAY_B_POEM[0]),
            "relay a is already broadcasting relay b's line"
        );
        assert!(
            !relay_b
                .create_payload(now)?
                .messages
                .iter()
                .any(|message| message.line == RELAY_A_POEM[0]),
            "relay b is already broadcasting relay a's line"
        );

        let later = Utc::now() + Duration::hours(1);

        assert!(
            relay_a
                .create_payload(later)?
                .messages
                .iter()
                .any(|message| message.line == RELAY_B_POEM[0]),
            "relay a is not broadcasting relay b's line"
        );
        assert!(
            relay_b
                .create_payload(later)?
                .messages
                .iter()
                .any(|message| message.line == RELAY_A_POEM[0]),
            "relay b is not broadcasting relay a's line"
        );

        Ok(())
    }
}

use std::collections::HashSet;

use ed25519_dalek::{Signature, SigningKey, VerifyingKey, ed25519::signature::SignerMut};
use serde::Deserialize;
use serde_json::value::RawValue;
use thiserror::Error;

use crate::{
    crypto::PublicKey,
    message::{Envelope, Message, RelayID, TrustedEnvelopeBundle},
};

// pub struct Payload {
//     me: RelayID,
//     signature: String,
//     envelopes: Vec<Envelope>,
// }

// #[derive(Error, Debug)]
// pub enum ReceivePayloadError {
//     #[error("verifying key is not trusted by relay")]
//     KeyNotTrusted,
//     #[error("cannot encode message into bytes")]
//     CannotEncodeMessage,
//     #[error("cannot verify message with given verifying key and signature")]
//     CannotVerifyMessage,
// }

// #[derive(Error, Debug)]
// pub enum CreatePayloadError {
//     #[error("cannot encode message into bytes")]
//     CannotEncodeMessage,
// }

// #[derive(Error, Debug)]
// pub enum PublicKeyError {
//     #[error("cannot read key")]
//     CannotReadKey,
// }

// pub fn receive_payload(
//     payload: &Payload,
//     trusted_keys: HashSet<PublicKey>,
// ) -> Result<(), ReceivePayloadError> {
//     if !trusted_keys.contains(&payload.verifying_key) {
//         return Err(ReceivePayloadError::KeyNotTrusted);
//     }

//     let message_bytes = get_messages_bytes(&payload.messages)
//         .map_err(|_| ReceivePayloadError::CannotEncodeMessage)?;

//     match payload
//         .verifying_key
//         .verify_strict(&message_bytes, &payload.signature)
//     {
//         Ok(_) => Ok(()),
//         Err(_) => Err(ReceivePayloadError::CannotVerifyMessage),
//     }
// }

// pub fn sign_messages(
//     messages: &Vec<Message>,
//     signing_key: &mut SigningKey,
// ) -> Result<Signature, CreatePayloadError> {
//     let message_bytes =
//         get_messages_bytes(messages).map_err(|_| CreatePayloadError::CannotEncodeMessage)?;

//     Ok(signing_key.sign(&message_bytes))
// }

// fn get_messages_bytes(messages: &Vec<Message>) -> Result<Vec<u8>, bincode::error::EncodeError> {
//     bincode::encode_to_vec(messages.clone(), bincode::config::standard())
// }

#[derive(Error, Debug)]
#[error("cannot deserialize json payload")]
pub struct UntrustedPayloadJSONError;

#[derive(Deserialize)]
pub struct UntrustedPayload<'a> {
    pub me: RelayID,
    pub signature: String,
    #[serde(borrow)]
    envelopes: &'a RawValue,
}

impl<'a> UntrustedPayload<'a> {
    pub fn from_json(json_str: &'a str) -> Result<Self, UntrustedPayloadJSONError> {
        serde_json::from_str(json_str).map_err(|_| UntrustedPayloadJSONError)
    }

    pub fn get_trusted_envelope_bundle(&self) -> TrustedEnvelopeBundle {}

    fn get_envelopes_bytes(&self) -> Vec<u8> {}
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_untrusted_payload() {
        let raw_json = include_str!("test_payload.json");
        let untrusted_payload_object = UntrustedPayload::from_json(raw_json);
        println!("{}", untrusted_payload_object.envelopes)
    }
}

use std::str::FromStr;

use anyhow::Result;
use json_syntax::{Print, Value};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;

use crate::{
    crypto::PublicKey,
    mailroom::OutgoingEnvelopes,
    message::{Envelope, RelayID},
};

#[derive(Error, Debug)]
pub enum VerifyPayloadError {
    #[error("public key is malformed")]
    MalformedPublicKey,
    #[error("public key is not trusted")]
    PublicKeyNotTrusted,
    #[error("cannot parse json")]
    CannotParseJson,
    #[error("cannot verify message with given public key and signature")]
    CannotVerifyMessage,
}

#[derive(Error, Debug)]
#[error("cannot deserialize json payload")]
pub struct UntrustedPayloadJSONError;

#[derive(Deserialize)]
pub struct UnverifiedPayload<'a> {
    from: RelayID,
    signature: String,
    #[serde(rename(deserialize = "envelopes"))]
    #[serde(borrow)]
    envelopes_raw_value: &'a RawValue,
}

impl<'a> UnverifiedPayload<'a> {
    pub fn from_json(json_str: &'a str) -> Result<Self, UntrustedPayloadJSONError> {
        serde_json::from_str(json_str).map_err(|_| UntrustedPayloadJSONError)
    }

    pub fn verify<I>(self, trusted_public_keys: I) -> Result<VerifiedPayload, VerifyPayloadError>
    where
        I: IntoIterator<Item = PublicKey>,
    {
        let claimed_public_key = match PublicKey::new_from_b64(&self.from.key) {
            Ok(public_key) => public_key,
            Err(_) => return Err(VerifyPayloadError::MalformedPublicKey),
        };

        if !trusted_public_keys
            .into_iter()
            .any(|key| key == claimed_public_key)
        {
            return Err(VerifyPayloadError::PublicKeyNotTrusted);
        }

        let envelope_bytes = match get_canon_json_bytes(self.envelopes_raw_value.get()) {
            Ok(envelope_bytes) => envelope_bytes,
            Err(_) => return Err(VerifyPayloadError::CannotParseJson),
        };

        if let Err(_) = claimed_public_key.verify(envelope_bytes, &self.signature) {
            return Err(VerifyPayloadError::CannotVerifyMessage);
        }

        let verified_envelopes =
            match serde_json::from_str::<Vec<Envelope>>(self.envelopes_raw_value.get()) {
                Ok(envelope_vec) => envelope_vec,
                Err(_) => return Err(VerifyPayloadError::CannotParseJson),
            };

        Ok(VerifiedPayload {
            from: self.from,
            public_key: claimed_public_key,
            envelopes: verified_envelopes,
        })
    }
}

pub struct VerifiedPayload {
    pub(crate) public_key: PublicKey,
    pub(crate) from: RelayID,
    pub(crate) envelopes: Vec<Envelope>,
}

impl VerifiedPayload {
    pub fn from(&self) -> &RelayID {
        &self.from
    }

    pub fn envelopes(&self) -> &Vec<Envelope> {
        &self.envelopes
    }
}

#[derive(Error, Debug)]
pub enum CreatePayloadError {
    #[error("cannot serialize into json for some reason")]
    CannotSerializeJson,
    #[error("cannot sign json for some reason")]
    CannotSignJson,
}

impl OutgoingEnvelopes {
    pub fn create_payload(&self) -> Result<String, CreatePayloadError> {
        let envelopes_json = serde_json::to_string(&self.envelopes)
            .map_err(|_| CreatePayloadError::CannotSerializeJson)?;

        let envelopes_bytes = get_canon_json_bytes(&envelopes_json)
            .map_err(|_| CreatePayloadError::CannotSerializeJson)?;

        let signature = self
            .secret_key
            .clone()
            .sign(&envelopes_bytes)
            .map_err(|_| CreatePayloadError::CannotSignJson)?;

        let outgoing_payload = OutgoingPayload {
            from: &self.relay_id,
            signature: signature,
            envelopes: &self.envelopes,
        };

        let payload_json = serde_json::to_string(&outgoing_payload)
            .map_err(|_| CreatePayloadError::CannotSerializeJson)?;

        Ok(payload_json)
    }
}

#[derive(Serialize)]
struct OutgoingPayload<'a> {
    from: &'a RelayID,
    signature: String,
    envelopes: &'a Vec<Envelope>,
}

fn get_canon_json_bytes(json_string: &str) -> Result<Vec<u8>> {
    let mut value = Value::from_str(json_string)?;

    value.canonicalize();

    Ok(value.compact_print().to_string().as_bytes().into())
}

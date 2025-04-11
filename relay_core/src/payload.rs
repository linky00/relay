use std::{collections::HashSet, str::FromStr};

use anyhow::Result;
use json_syntax::{Print, Value};
use serde::Deserialize;
use serde_json::value::RawValue;
use thiserror::Error;

use crate::{
    crypto::{PublicKey, SecretKey},
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
    me: RelayID,
    signature: String,
    #[serde(rename(deserialize = "envelopes"))]
    #[serde(borrow)]
    envelopes_serde_value: &'a RawValue,
}

impl<'a> UnverifiedPayload<'a> {
    pub fn from_json(json_str: &'a str) -> Result<Self, UntrustedPayloadJSONError> {
        serde_json::from_str(json_str).map_err(|_| UntrustedPayloadJSONError)
    }

    pub fn verify<I>(self, trusted_public_keys: I) -> Result<VerifiedPayload, VerifyPayloadError>
    where
        I: IntoIterator<Item = PublicKey>,
    {
        let claimed_public_key = match PublicKey::new_from_b64(&self.me.key) {
            Ok(public_key) => public_key,
            Err(_) => return Err(VerifyPayloadError::MalformedPublicKey),
        };

        if !trusted_public_keys
            .into_iter()
            .any(|key| key == claimed_public_key)
        {
            return Err(VerifyPayloadError::PublicKeyNotTrusted);
        }

        let envelope_bytes = match get_canon_json_bytes(self.envelopes_serde_value.get()) {
            Ok(envelope_bytes) => envelope_bytes,
            Err(_) => return Err(VerifyPayloadError::CannotParseJson),
        };

        if let Err(_) = claimed_public_key.verify(envelope_bytes, &self.signature) {
            return Err(VerifyPayloadError::CannotVerifyMessage);
        }

        let verified_envelopes =
            match serde_json::from_str::<Vec<Envelope>>(self.envelopes_serde_value.get()) {
                Ok(envelope_vec) => envelope_vec,
                Err(_) => return Err(VerifyPayloadError::CannotParseJson),
            };

        Ok(VerifiedPayload::new(self, verified_envelopes))
    }
}

pub struct VerifiedPayload {
    me: RelayID,
    signature: String,
    envelopes: Vec<Envelope>,
}

impl VerifiedPayload {
    fn new(unverified_payload: UnverifiedPayload, envelopes: Vec<Envelope>) -> Self {
        VerifiedPayload {
            me: unverified_payload.me,
            signature: unverified_payload.signature,
            envelopes,
        }
    }
}

fn get_canon_json_bytes(json_string: &str) -> Result<Vec<u8>> {
    let mut value = Value::from_str(json_string)?;

    value.canonicalize();

    Ok(value.compact_print().to_string().as_bytes().into())
}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;

use crate::{
    crypto::{PublicKey, get_canon_json_bytes},
    mailroom::OutgoingEnvelopes,
    message::{Certificate, Envelope, Message},
};

#[derive(Error, Debug)]
pub enum UntrustedPayloadError {
    #[error("public key in certificate of payload or message is malformed")]
    MalformedPublicKey,
    #[error("public key in certificate of payload is not trusted")]
    PublicKeyNotTrusted,
    #[error("cannot parse json")]
    CannotParseJson,
    #[error("cannot verify payload certificate")]
    CannotVerify,
}

#[derive(Deserialize)]
pub struct UntrustedPayload<'a> {
    certificate: Certificate,
    #[serde(rename(deserialize = "envelopes"))]
    #[serde(borrow)]
    envelopes_raw_value: &'a RawValue,
}

impl<'a> UntrustedPayload<'a> {
    pub fn from_json(json_str: &'a str) -> Result<Self, UntrustedPayloadError> {
        serde_json::from_str(json_str).map_err(|_| UntrustedPayloadError::CannotParseJson)
    }

    pub fn try_trust<I>(
        self,
        trusted_public_keys: I,
    ) -> Result<TrustedPayload, UntrustedPayloadError>
    where
        I: IntoIterator<Item = PublicKey>,
    {
        let Ok(claimed_public_key) = PublicKey::new_from_b64(&self.certificate.key) else {
            return Err(UntrustedPayloadError::MalformedPublicKey);
        };

        if !trusted_public_keys
            .into_iter()
            .any(|key| key == claimed_public_key)
        {
            return Err(UntrustedPayloadError::PublicKeyNotTrusted);
        }

        check_signature(
            &self.certificate.signature,
            claimed_public_key,
            self.envelopes_raw_value,
        )?;

        let unverified_envelopes: Vec<UnverifiedEnvelope> =
            serde_json::from_str(self.envelopes_raw_value.get())
                .map_err(|_| UntrustedPayloadError::CannotParseJson)?;

        let mut envelopes = vec![];
        let mut unverified_messages_count = 0;

        for unverified_envelope in unverified_envelopes {
            match check_signature(
                &unverified_envelope.unverified_message.certificate.signature,
                PublicKey::new_from_b64(&unverified_envelope.unverified_message.certificate.key)
                    .map_err(|_| UntrustedPayloadError::MalformedPublicKey)?,
                unverified_envelope.unverified_message.contents_raw_json,
            ) {
                Ok(()) => envelopes.push(Envelope {
                    forwarded: unverified_envelope.forwarded,
                    ttl: unverified_envelope.ttl,
                    message: Message {
                        certificate: unverified_envelope.unverified_message.certificate,
                        contents: serde_json::from_str(
                            unverified_envelope
                                .unverified_message
                                .contents_raw_json
                                .get(),
                        )
                        .map_err(|_| UntrustedPayloadError::CannotParseJson)?,
                    },
                }),
                Err(UntrustedPayloadError::CannotVerify) => unverified_messages_count += 1,
                Err(e) => return Err(e),
            }
        }

        Ok(TrustedPayload {
            public_key: claimed_public_key,
            certificate: self.certificate,
            envelopes,
            unverified_messages_count,
        })
    }
}

#[derive(Deserialize)]
struct UnverifiedEnvelope<'a> {
    forwarded: Vec<String>,
    ttl: u8,
    #[serde(rename(deserialize = "message"))]
    #[serde(borrow)]
    unverified_message: UnverifiedMessage<'a>,
}

#[derive(Deserialize)]
struct UnverifiedMessage<'a> {
    certificate: Certificate,
    #[serde(rename(deserialize = "contents"))]
    #[serde(borrow)]
    contents_raw_json: &'a RawValue,
}

pub struct TrustedPayload {
    pub(crate) public_key: PublicKey,
    pub(crate) certificate: Certificate,
    pub(crate) envelopes: Vec<Envelope>,
    pub(crate) unverified_messages_count: u32,
}

impl TrustedPayload {
    pub fn certificate(&self) -> &Certificate {
        &self.certificate
    }

    pub fn envelopes(&self) -> &Vec<Envelope> {
        &self.envelopes
    }

    pub fn unverified_messages_count(&self) -> u32 {
        self.unverified_messages_count
    }
}

impl OutgoingEnvelopes {
    pub fn create_payload(&self) -> String {
        let envelopes_json = serde_json::to_string(&self.envelopes)
            .expect("should be able to serialize any envelopes to json");

        let envelopes_bytes = get_canon_json_bytes(&envelopes_json)
            .expect("should be able to get canon bytes for any json string");

        let signature = self.secret_key.clone().sign(&envelopes_bytes);

        let outgoing_payload = OutgoingPayload {
            certificate: Certificate {
                key: self.secret_key.public_key().to_string(),
                signature,
            },
            envelopes: &self.envelopes,
        };

        serde_json::to_string(&outgoing_payload)
            .expect("should be able to serialize any payload to json")
    }
}

#[derive(Serialize)]
struct OutgoingPayload<'a> {
    certificate: Certificate,
    envelopes: &'a Vec<Envelope>,
}

fn check_signature(
    signature: &str,
    key: PublicKey,
    raw_value: &RawValue,
) -> Result<(), UntrustedPayloadError> {
    let bytes = get_canon_json_bytes(raw_value.get())
        .map_err(|_| UntrustedPayloadError::CannotParseJson)?;

    key.verify(&bytes, signature)
        .map_err(|_| UntrustedPayloadError::CannotVerify)?;

    Ok(())
}

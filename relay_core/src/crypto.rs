use anyhow::Result;
use base64::{Engine, prelude::BASE64_STANDARD};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use thiserror::Error;

pub const PUBLIC_KEY_LENGTH: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;
pub const SECRET_KEY_LENGTH: usize = ed25519_dalek::SECRET_KEY_LENGTH;

#[derive(Error, Debug)]
pub enum NewKeyError {
    #[error("cannot read base64 in standard alphabet from input")]
    CannotDecode,
    #[error("base64 string decodes to incorrect number of bytes")]
    IncorrectLength,
    #[error("invalid ed21556 key")]
    InvalidKey,
}

#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    pub fn new_from_b64<S: AsRef<str>>(b64_string: S) -> Result<Self, NewKeyError> {
        match bytes_from_b64(b64_string) {
            Ok(bytes) => Self::new_from_bytes(&bytes),
            Err(e) => Err(e),
        }
    }

    pub fn new_from_bytes(bytes: &[u8; PUBLIC_KEY_LENGTH]) -> Result<Self, NewKeyError> {
        match VerifyingKey::from_bytes(&bytes) {
            Ok(verifying_key) => Ok(Self(verifying_key)),
            Err(_) => Err(NewKeyError::InvalidKey),
        }
    }

    pub fn verify<S: AsRef<str>>(&self, message: Vec<u8>, signature: S) -> Result<()> {
        let signature_bytes = bytes_from_b64(signature)?;
        let signature = Signature::from_bytes(&signature_bytes);
        self.0.verify_strict(&message, &signature)?;

        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SecretKey(SigningKey);

impl SecretKey {
    pub fn new_from_b64<S: AsRef<str>>(b64_string: S) -> Result<Self, NewKeyError> {
        bytes_from_b64(b64_string).map(|bytes| Self::new_from_bytes(&bytes))
    }

    pub fn new_from_bytes(bytes: &[u8; SECRET_KEY_LENGTH]) -> Self {
        Self(SigningKey::from_bytes(bytes))
    }
}

fn bytes_from_b64<S: AsRef<str>, const N: usize>(b64_string: S) -> Result<[u8; N], NewKeyError> {
    match BASE64_STANDARD.decode(b64_string.as_ref()) {
        Ok(bytes_vec) => match bytes_vec.try_into() {
            Ok(bytes_array) => Ok(bytes_array),
            Err(_) => Err(NewKeyError::IncorrectLength),
        },
        Err(_) => Err(NewKeyError::CannotDecode),
    }
}

fn b64_from_bytes(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

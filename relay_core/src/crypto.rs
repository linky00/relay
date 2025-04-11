use base64::{Engine, prelude::BASE64_STANDARD};
use thiserror::Error;

pub const PUBLIC_KEY_LENGTH: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;
pub const SECRET_KEY_LENGTH: usize = ed25519_dalek::SECRET_KEY_LENGTH;
pub const SIGNATURE_LENGTH: usize = ed25519_dalek::SIGNATURE_LENGTH;

#[derive(Error, Debug)]
pub enum B64KeyError {
    #[error("cannot read base64 in standard alphabet from input")]
    CannotDecode,
    #[error("base64 string decodes to incorrect number of bytes")]
    IncorrectLength,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
struct PublicKey([u8; PUBLIC_KEY_LENGTH]);

impl PublicKey {
    pub fn new_from_b64<S: AsRef<str>>(b64_string: S) -> Result<Self, B64KeyError> {
        bytes_from_b64(b64_string).map(|bytes| Self(bytes))
    }

    pub fn new_from_bytes(bytes: [u8; PUBLIC_KEY_LENGTH]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct SecretKey([u8; SECRET_KEY_LENGTH]);

impl SecretKey {
    pub fn new_from_b64<S: AsRef<str>>(b64_string: S) -> Result<Self, B64KeyError> {
        bytes_from_b64(b64_string).map(|bytes| Self(bytes))
    }

    pub fn new_from_bytes(bytes: [u8; SECRET_KEY_LENGTH]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Signature([u8; SIGNATURE_LENGTH]);

impl Signature {
    pub fn new_from_b64<S: AsRef<str>>(b64_string: S) -> Result<Self, B64KeyError> {
        bytes_from_b64(b64_string).map(|bytes| Self(bytes))
    }

    pub fn new_from_bytes(bytes: [u8; SIGNATURE_LENGTH]) -> Self {
        Self(bytes)
    }
}

fn bytes_from_b64<S: AsRef<str>, const N: usize>(b64_string: S) -> Result<[u8; N], B64KeyError> {
    match BASE64_STANDARD.decode(b64_string.as_ref()) {
        Ok(bytes_vec) => match bytes_vec.try_into() {
            Ok(bytes_array) => Ok(bytes_array),
            Err(_) => Err(B64KeyError::IncorrectLength),
        },
        Err(_) => Err(B64KeyError::CannotDecode),
    }
}

fn b64_from_bytes(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

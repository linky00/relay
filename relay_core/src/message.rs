use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub forwarded: Vec<RelayID>,
    pub ttl: u8,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Message {
    pub uuid: Uuid,
    pub line: String,
    pub author: RelayID,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Hash)]
pub struct RelayID {
    pub key: String,
    pub name: String,
}

#[derive(Error, Debug)]
#[error("cannot create uuid from this string")]
pub struct UuidFromStringError;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Uuid {
    internal: uuid::Uuid,
}

impl Uuid {
    pub fn new() -> Self {
        Self {
            internal: uuid::Uuid::new_v4(),
        }
    }

    pub fn from_string<S: AsRef<str>>(string: S) -> Result<Self, UuidFromStringError> {
        match uuid::Uuid::from_str(string.as_ref()) {
            Ok(uuid) => Ok(Self { internal: uuid }),
            Err(_) => Err(UuidFromStringError),
        }
    }

    pub fn as_string(&self) -> String {
        self.internal.hyphenated().to_string()
    }
}

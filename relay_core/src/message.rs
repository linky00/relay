use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub forwarded: Vec<RelayID>,
    pub ttl: u8,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Message {
    pub(crate) uuid: String,
    pub(crate) line: String,
    pub(crate) author: RelayID,
}

impl Message {
    pub(crate) fn new(line: String, author: RelayID) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4().hyphenated().to_string(),
            line,
            author,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Hash)]
pub struct RelayID {
    pub(crate) key: String,
    pub(crate) name: String,
}

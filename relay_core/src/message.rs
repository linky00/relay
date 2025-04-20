use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub forwarded: Vec<String>,
    pub ttl: u8,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Message {
    pub certificate: Certificate,
    pub contents: MessageContents,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MessageContents {
    pub uuid: String,
    pub author: String,
    pub line: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Certificate {
    pub key: String,
    pub signature: String,
}

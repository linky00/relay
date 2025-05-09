use std::str::FromStr;

use relay_core::crypto::PublicKey;
use reqwest::Url;
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaemonConfig {
    pub trusted_relays: Vec<RelayData>,
    pub custom_initial_ttl: Option<u8>,
    pub custom_max_forwarding_ttl: Option<u8>,
}

impl DaemonConfig {
    pub(crate) fn trusted_public_keys(&self) -> Vec<PublicKey> {
        self.trusted_relays.iter().map(|relay| relay.key).collect()
    }
}

#[derive(Clone)]
pub struct ListenerConfig {
    pub custom_port: Option<u16>,
}

#[derive(Error, Debug)]
pub enum RelayDataError {
    #[error("url is not valid (is it missing http/https?)")]
    UrlNotValid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayData {
    pub key: PublicKey,
    pub nickname: Option<String>,
    pub(crate) endpoint: Option<Url>,
}

impl RelayData {
    pub fn new(
        key: PublicKey,
        nickname: Option<String>,
        listener_endpoint: Option<&str>,
    ) -> Result<Self, RelayDataError> {
        let endpoint = match listener_endpoint {
            Some(url_str) => Some(Url::from_str(url_str).map_err(|_| RelayDataError::UrlNotValid)?),
            None => None,
        };

        Ok(RelayData {
            key,
            nickname,
            endpoint,
        })
    }

    pub fn endpoint(&self) -> Option<&Url> {
        self.endpoint.as_ref()
    }
}

impl Serialize for RelayData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("RelayData", 3)?;
        state.serialize_field("key", &self.key)?;
        state.serialize_field("nickname", &self.nickname)?;
        state.serialize_field(
            "endpoint",
            &self.endpoint.clone().map(|url| url.to_string()),
        )?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RelayData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RelayDataIntermediate {
            key: PublicKey,
            nickname: Option<String>,
            endpoint: Option<String>,
        }

        let intermediate = RelayDataIntermediate::deserialize(deserializer)?;

        let endpoint = if let Some(url_str) = intermediate.endpoint {
            Some(Url::from_str(&url_str).map_err(serde::de::Error::custom)?)
        } else {
            None
        };

        Ok(RelayData {
            key: intermediate.key,
            nickname: intermediate.nickname,
            endpoint,
        })
    }
}

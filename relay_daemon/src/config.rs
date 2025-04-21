use std::str::FromStr;

use relay_core::crypto::{PublicKey, SecretKey};
use reqwest::Url;
use thiserror::Error;

#[derive(Clone)]
pub struct Config {
    pub name: String,
    pub secret_key: SecretKey,
    pub trusted_relays: Vec<RelayData>,
    pub initial_ttl: Option<u8>,
    pub max_forwarding_ttl: Option<u8>,
}

impl Config {
    pub(crate) fn trusted_public_keys(&self) -> Vec<PublicKey> {
        self.trusted_relays.iter().map(|relay| relay.key).collect()
    }
}

#[derive(Error, Debug)]
pub enum RelayDataError {
    #[error("url is not valid (is it missing http/https?)")]
    UrlNotValid,
}

#[derive(Clone)]
pub struct RelayData {
    pub key: PublicKey,
    pub nickname: Option<String>,
    pub(crate) listener_endpoint: Option<Url>,
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
            listener_endpoint: endpoint,
        })
    }
}

pub trait GetConfig {
    fn get(&self) -> Option<&Config>;
}

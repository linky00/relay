use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Timelike, Utc};
use thiserror::Error;

use crate::{
    message::{Envelope, Message},
    payload::VerifiedPayload,
};

const INITIAL_TTL: u8 = 8;
const MAX_FORWARDING_TTL: u8 = 8;

#[derive(Error, Debug)]
pub enum ReceivePayloadError {
    #[error("already received payload from this key")]
    AlreadyReceivedFromKey,
}

pub struct Mailroom<A: Archive> {
    new_messages: HashSet<Message>,
    forwarding_received_this_hour: HashMap<String, Vec<Envelope>>,
    forwarding_received_last_hour: HashMap<String, Vec<Envelope>>,
    last_seen_time: Option<DateTime<Utc>>,
    config: Config,
    archive: A,
}

impl<A: Archive> Mailroom<A> {
    pub fn new(config: Config, archive: A) -> Self {
        Mailroom {
            new_messages: HashSet::new(),
            forwarding_received_this_hour: HashMap::new(),
            forwarding_received_last_hour: HashMap::new(),
            last_seen_time: None,
            config,
            archive,
        }
    }

    pub fn receive_payload(&mut self, payload: VerifiedPayload) -> Result<(), ReceivePayloadError> {
        self.receive_payload_at_time(payload, Utc::now())
    }

    #[cfg(feature = "chrono")]
    pub fn receive_payload_at_time(
        &mut self,
        payload: VerifiedPayload,
        now: DateTime<Utc>,
    ) -> Result<(), ReceivePayloadError> {
        self.handle_time(now);

        let from_key = payload.from.key;

        if self.forwarding_received_this_hour.contains_key(&from_key) {
            return Err(ReceivePayloadError::AlreadyReceivedFromKey);
        }

        let mut forwarding_from_this_key = vec![];

        for envelope in payload.envelopes {
            self.archive.add_envelope_to_archive(&envelope);

            if self.new_messages.contains(&envelope.message) {
                forwarding_from_this_key.push(envelope);
            } else if !self.archive.is_message_in_archive(&envelope.message) {
                self.new_messages.insert(envelope.message.clone());
                forwarding_from_this_key.push(envelope);
            }
        }

        self.forwarding_received_this_hour
            .insert(from_key, forwarding_from_this_key);

        Ok(())
    }

    fn handle_time(&mut self, now: DateTime<Utc>) {
        if let Some(last_seen_time) = self.last_seen_time {
            let on_the_hour = |datetime: DateTime<Utc>| {
                datetime
                    .with_minute(0)
                    .expect("should be able to set any utc time to minute 0")
                    .with_second(0)
                    .expect("should be able to set any utc time to second 0")
                    .with_nanosecond(0)
                    .expect("should be able to set any utc time to nanosecond 0")
            };

            let now_oth = on_the_hour(now);
            let last_seen_oth = on_the_hour(last_seen_time);

            if now_oth == last_seen_oth + Duration::hours(1) {
                self.new_messages = HashSet::new();
                self.forwarding_received_last_hour = self.forwarding_received_this_hour.clone();
                self.forwarding_received_this_hour = HashMap::new();
            } else if now_oth != last_seen_oth {
                self.new_messages = HashSet::new();
                self.forwarding_received_last_hour = HashMap::new();
                self.forwarding_received_this_hour = HashMap::new();
            }
        }
        self.last_seen_time = Some(now);
    }
}

// impl Mailroom {
//     pub fn new<S, I>(name: S, poem: I) -> Self
//     where
//         S: Into<String>,
//         I: IntoIterator,
//         I::Item: AsRef<str>,
//     {
//         Self {
//             name: name.into(),
//             poem: poem.into_iter().map(|s| s.as_ref().to_owned()).collect(),
//             next_line_idx: 0,
//             outgoing_envelopes: vec![],
//             last_generated_at: None,
//             received_envelopes: HashMap::new(),
//             archived_messages: HashMap::new(),
//         }
//     }

//     pub fn receive_messages(&mut self, messages: &Vec<Message>) {
//         for received_message in messages {
//             if self
//                 .archived_messages
//                 .keys()
//                 .any(|archived_uuid| *archived_uuid == received_message.uuid)
//             {
//                 continue;
//             }

//             self.received_envelopes
//                 .entry(received_message.uuid)
//                 .or_insert(received_message.clone());
//         }
//     }

//     pub fn get_messages(&mut self, now: DateTime<Utc>) -> Vec<Message> {
//         if let Some(last_generated_at) = self.last_generated_at {
//             if now.date_naive() != last_generated_at.date_naive()
//                 || now.hour() != last_generated_at.hour()
//             {
//                 self.regenerate_outgoing_messages(now);
//             }
//         } else {
//             self.regenerate_outgoing_messages(now);
//         }

//         self.outgoing_envelopes.clone()
//     }

//     fn regenerate_outgoing_messages(&mut self, now: DateTime<Utc>) {
//         self.outgoing_envelopes = self.received_envelopes.values().cloned().collect();

//         let new_message = Message {
//             uuid: Uuid::new_v4(),
//             line: self.poem[self.next_line_idx % self.poem.len()].clone(),
//             author: self.name.clone(),
//             ttl: INITIAL_TTL,
//         };
//         self.next_line_idx += 1;
//         self.outgoing_envelopes.push(new_message);

//         self.outgoing_envelopes.iter_mut().for_each(|message| {
//             message.ttl -= 1;
//         });
//         self.outgoing_envelopes.retain(|message| message.ttl > 0);

//         for (old_uuid, old_message) in self.received_envelopes.drain() {
//             self.archived_messages.insert(old_uuid, old_message);
//         }

//         self.last_generated_at = Some(now);
//     }
// }

pub struct Config {
    pub name: String,
    pub initial_ttl: u8,
    pub max_forwarding_ttl: u8,
}

impl Config {
    pub fn new<S: AsRef<str>>(name: S) -> Config {
        Config {
            name: name.as_ref().into(),
            initial_ttl: INITIAL_TTL,
            max_forwarding_ttl: MAX_FORWARDING_TTL,
        }
    }
}

pub trait Archive {
    fn is_message_in_archive(&self, message: &Message) -> bool;

    fn add_envelope_to_archive(&mut self, envelope: &Envelope);
}

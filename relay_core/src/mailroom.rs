use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};

use crate::message::{Envelope, Message, RelayID, Uuid};

const INITIAL_TTL: u8 = 10;

struct Mailroom {
    name: String,
    outgoing_envelopes: HashMap<RelayID, Envelope>,
    last_generated_at: Option<DateTime<Utc>>,
    received_envelopes: HashMap<Uuid, Message>,
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

trait MessageArchive {
    fn is_message_in_archive(uuid: Uuid) -> bool;

    fn add_message_to_archive(message: Message);
}

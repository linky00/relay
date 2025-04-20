use relay_core::message::Envelope;

use crate::config::RelayData;

pub enum Event {
    SentToHost(RelayData, Vec<Envelope>),
    ProblemSendingToHost(RelayData, String),
    ReceivedFromHost(RelayData, Vec<Envelope>),
    AlreadyReceivedFromHost(RelayData),
    HttpErrorResponseFromHost(RelayData, String),
    BadResponseFromHost(RelayData),
}

pub trait HandleEvent {
    fn handle_event(&mut self, event: Event);
}

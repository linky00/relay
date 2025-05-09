use relay_core::message::{Envelope, Message};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::RelayData;

pub enum Event {
    ListenerStartedListening(u16),
    ListenerReceivedFromSender(Option<RelayData>, Vec<Envelope>),
    ListenerSentToSender(Option<RelayData>, Vec<Envelope>),
    ListenerReceivedBadPayload,
    ListenerReceivedFromUntrustedSender,
    ListenerDBError(String),
    ListenerAlreadyReceivedFromSender(Option<RelayData>),
    SenderStartedSchedule,
    SenderBeginningRun,
    SenderDBError(String),
    SenderSentToListener(RelayData, Vec<Envelope>),
    SenderReceivedFromListener(RelayData, Vec<Envelope>),
    SenderFailedSending(RelayData, String),
    SenderReceivedHttpError(RelayData, String),
    SenderReceivedBadResponse(RelayData),
    SenderAlreadyReceivedFromListener(RelayData),
    SenderFinishedRun,
    AddedMessageToArchive(Message),
}

pub type EventSender = UnboundedSender<Event>;

use std::sync::Arc;

use relay_core::message::Envelope;
use tokio::sync::Mutex;

use crate::config::RelayData;

pub enum Event {
    ListenerStartedListening(u16),
    ListenerReceivedFromSender(Option<RelayData>, Vec<Envelope>),
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
}

pub trait HandleEvent {
    fn handle_event(&mut self, event: Event);
}

pub(crate) async fn emit_event<E>(event_handler: &Arc<Mutex<E>>, event: Event)
where
    E: HandleEvent + Send + 'static,
{
    event_handler.lock().await.handle_event(event);
}

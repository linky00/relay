use std::sync::Arc;

use relay_core::message::Envelope;
use tokio::sync::Mutex;

use crate::config::RelayData;

pub enum Event {
    StartedListener(u16),
    StartedSenderSchedule,
    BeginningSendingToListeners,
    SentToListener(RelayData, Vec<Envelope>),
    ReceivedFromListener(RelayData, Vec<Envelope>),
    ProblemSendingToListener(RelayData, String),
    HttpErrorResponseFromListener(RelayData, String),
    BadResponseFromListener(RelayData),
    AlreadyReceivedFromListener(RelayData),
    FinishedSendingToListeners,
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

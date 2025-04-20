use std::sync::Arc;

use archive::MockArchive;
use relay_core::mailroom::Mailroom;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{config::GetConfig, event::HandleEvent, line::GetLine};

mod archive;
mod exchange;

pub struct Daemon<L, C, E> {
    state: Arc<DaemonState<L, C, E>>,
    fast_mode: bool,
}

impl<L, C, E> Daemon<L, C, E>
where
    L: GetLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
    E: HandleEvent + Sync + Send + 'static,
{
    pub fn new(line_generator: L, config_reader: C, event_handler: E) -> Self {
        Self {
            state: Arc::new(DaemonState::new(
                line_generator,
                config_reader,
                event_handler,
            )),
            fast_mode: false,
        }
    }

    pub fn fast(self) -> Self {
        Self {
            fast_mode: true,
            ..self
        }
    }

    pub async fn start_sending_to_hosts(&self) {
        let scheduler = JobScheduler::new().await.unwrap();

        let state_clone = self.state.clone();
        let fast_mode = self.fast_mode;
        scheduler
            .add(
                Job::new_async(
                    match self.fast_mode {
                        true => "*/5 * * * * *",
                        false => "0 * * * *",
                    },
                    move |_, _| {
                        let state_clone = state_clone.clone();
                        Box::pin(async move {
                            if let Some(config) = state_clone.config_reader.get() {
                                exchange::send_to_hosts(
                                    state_clone.mailroom.clone(),
                                    state_clone.line_generator.lock().await.get(),
                                    config,
                                    state_clone.event_handler.clone(),
                                    fast_mode,
                                )
                                .await;
                            }
                        })
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();

        scheduler.start().await.unwrap();
    }
}

struct DaemonState<L, C, E> {
    mailroom: Arc<Mutex<Mailroom<MockArchive>>>,
    line_generator: Mutex<L>,
    config_reader: C,
    event_handler: Arc<Mutex<E>>,
}

impl<L, C, E> DaemonState<L, C, E>
where
    L: GetLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
    E: HandleEvent + Sync + Send + 'static,
{
    fn new(line_generator: L, config_reader: C, event_handler: E) -> Self {
        Self {
            mailroom: Arc::new(Mutex::new(Mailroom::new(MockArchive::new()))),
            line_generator: Mutex::new(line_generator),
            config_reader,
            event_handler: Arc::new(Mutex::new(event_handler)),
        }
    }
}

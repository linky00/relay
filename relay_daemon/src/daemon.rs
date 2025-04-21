use std::{sync::Arc, time::Duration};

use archive::MockArchive;
use chrono::{DateTime, Timelike, Utc};
use relay_core::mailroom::{GetNextLine, Mailroom};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{config::GetConfig, event::HandleEvent};

mod archive;
mod exchange;

pub struct Daemon<L: GetNextLine, C, E> {
    state: Arc<DaemonState<L, C, E>>,
    fast_mode: bool,
}

impl<L, C, E> Daemon<L, C, E>
where
    L: GetNextLine + Sync + Send + 'static,
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

    pub fn new_fast(line_generator: L, config_reader: C, event_handler: E) -> Self {
        let mut daemon = Self::new(line_generator, config_reader, event_handler);
        daemon.fast_mode = true;
        daemon
    }

    pub async fn start_sending_to_hosts(&self) {
        let scheduler = JobScheduler::new().await.unwrap();

        let state_clone = self.state.clone();
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
                                    config,
                                    state_clone.event_handler.clone(),
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

struct DaemonState<L: GetNextLine, C, E> {
    mailroom: Arc<Mutex<Mailroom<L, MockArchive>>>,
    config_reader: C,
    event_handler: Arc<Mutex<E>>,
}

impl<L, C, E> DaemonState<L, C, E>
where
    L: GetNextLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
    E: HandleEvent + Sync + Send + 'static,
{
    fn new(line_generator: L, config_reader: C, event_handler: E) -> Self {
        let flatten_time = |datetime: DateTime<Utc>| {
            datetime
                .with_second(datetime.second() / 5 * 5)
                .expect("should be able to set seconds to a multiple of 5")
                .with_nanosecond(0)
                .expect("should be able to set any utc time to nanosecond 0")
        };
        let interval = Duration::from_secs(5);

        Self {
            mailroom: Arc::new(Mutex::new(Mailroom::new_with_custom_time(
                line_generator,
                MockArchive::new(),
                flatten_time,
                interval,
            ))),
            config_reader,
            event_handler: Arc::new(Mutex::new(event_handler)),
        }
    }
}

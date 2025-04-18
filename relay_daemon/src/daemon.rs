use std::sync::Arc;

use relay_core::mailroom::Mailroom;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{archive::MockArchive, config::GetConfig, line::GetLine};

mod exchange;

pub struct Daemon<L, C> {
    state: Arc<DaemonState<L, C>>,
    fast_mode: bool,
}

impl<L, C> Daemon<L, C>
where
    L: GetLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
{
    pub fn new(line: L, config: C) -> Self {
        Self {
            state: Arc::new(DaemonState::new(line, config)),
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
                        true => "* * * * * *",
                        false => "0 * * * *",
                    },
                    move |_, _| {
                        let state_clone = state_clone.clone();
                        Box::pin(async move {
                            if let Some(config) = state_clone.config.get() {
                                exchange::send_to_hosts(
                                    &state_clone.mailroom,
                                    state_clone.line.get(),
                                    config,
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

struct DaemonState<L, C> {
    mailroom: Mutex<Mailroom<MockArchive>>,
    line: L,
    config: C,
}

impl<L, C> DaemonState<L, C>
where
    L: GetLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
{
    fn new(line: L, config: C) -> Self {
        Self {
            mailroom: Mutex::new(Mailroom::new(MockArchive::new())),
            line,
            config,
        }
    }
}

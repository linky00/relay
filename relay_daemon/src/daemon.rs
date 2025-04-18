use std::{sync::Arc, time::Duration};

use relay_core::mailroom::Mailroom;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{archive::MockArchive, config::GetConfig, line::GetLine};

pub struct RelayDaemon<C: GetConfig, L: GetLine> {
    mailroom: Mailroom<MockArchive>,
    config: C,
    line: L,
    fast_mode: bool,
}

impl<C, L> RelayDaemon<C, L>
where
    L: GetLine + Sync + Send + 'static,
    C: GetConfig + Sync + Send + 'static,
{
    pub fn new(line: L, config: C) -> Self {
        Self {
            mailroom: Mailroom::new(MockArchive::new()),
            config,
            line,
            fast_mode: false,
        }
    }

    pub fn fast(self) -> Self {
        Self {
            fast_mode: true,
            ..self
        }
    }

    pub async fn start_sending_to_hosts(self: &Arc<Self>) {
        let sched = JobScheduler::new().await.unwrap();

        let send_to_hosts_schedule = match self.fast_mode {
            true => "* * * * * *",
            false => "0 * * * *",
        };
        let self_clone = self.clone();
        sched
            .add(
                Job::new(send_to_hosts_schedule, move |_, _| {
                    self_clone.send_to_hosts();
                })
                .unwrap(),
            )
            .await
            .unwrap();

        sched.start().await.unwrap();
    }

    fn send_to_hosts(&self) {
        println!("sending to hosts");
    }
}

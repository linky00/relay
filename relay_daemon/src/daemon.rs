use std::{sync::Arc, time::Duration};

use relay_core::mailroom::{Archive, Mailroom};
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::config::ReadConfig;

pub struct RelayDaemon<C: ReadConfig> {
    mailroom: Mailroom<DBArchive>,
    config: C,
    line_output: LineOutput,
}

impl<C: ReadConfig + Sync + Send + 'static> RelayDaemon<C> {
    pub fn new(config: C) -> Self {
        Self {
            mailroom: Mailroom::new(DBArchive),
            config,
            line_output: LineOutput::None,
        }
    }

    pub async fn start(self: Arc<Self>) {
        let sched = JobScheduler::new().await.unwrap();

        let self_clone = self.clone();
        sched
            .add(
                Job::new("* * * * * *", move |_, _| {
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

pub struct DBArchive;

impl Archive for DBArchive {
    fn add_envelope_to_archive(
        &mut self,
        from: &relay_core::message::RelayID,
        envelope: &relay_core::message::Envelope,
    ) {
        todo!()
    }

    fn is_message_in_archive(&self, message: &relay_core::message::Message) -> bool {
        todo!()
    }
}

enum LineOutput {
    Single(String),
    Loop { poem: Vec<String>, next_idx: usize },
    Once { poem: Vec<String>, next_idx: usize },
    None,
}

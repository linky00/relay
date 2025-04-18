use std::{sync::Arc, time::Duration};

use relay_core::{
    mailroom::{Archive, Mailroom},
    message::{Envelope, Message},
};
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::config::ReadConfig;

pub struct RelayDaemon<C: ReadConfig> {
    mailroom: Mailroom<DBArchive>,
    config: C,
    line_output: LineOutput,
    fast_mode: bool,
}

impl<C: ReadConfig + Sync + Send + 'static> RelayDaemon<C> {
    pub fn new(config: C) -> Self {
        Self {
            mailroom: Mailroom::new(DBArchive),
            config,
            line_output: LineOutput::None,
            fast_mode: false,
        }
    }

    pub fn new_fast(config: C) -> Self {
        let mut new = Self::new(config);
        new.fast_mode = true;
        new
    }

    pub async fn start(self: &Arc<Self>) {
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

pub struct DBArchive;

impl Archive for DBArchive {
    fn add_envelope_to_archive(&mut self, from: &str, envelope: &Envelope) {
        todo!()
    }

    fn is_message_in_archive(&self, message: &Message) -> bool {
        todo!()
    }
}

enum LineOutput {
    Single(String),
    Loop { poem: Vec<String>, next_idx: usize },
    Once(Vec<String>),
    None,
}

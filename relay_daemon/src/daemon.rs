use std::{sync::Arc, time::Duration};

use archive::MockArchive;
use axum::{Router, routing};
use chrono::{DateTime, Timelike, Utc};
use relay_core::{
    crypto::SecretKey,
    mailroom::{GetNextLine, Mailroom},
};
use thiserror::Error;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{config::GetConfig, event::HandleEvent};

mod archive;
mod exchange;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("cannot read config")]
    CannotReadConfig,
    #[error("cannot bind port {0} (is it in use?)")]
    CannotBindPort(u16),
    #[error("cannot start sender for some reason")]
    CannotStartSender,
}

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
    pub fn new(
        line_generator: L,
        secret_key: SecretKey,
        config_reader: C,
        event_handler: E,
    ) -> Self {
        Self {
            state: Arc::new(DaemonState::new(
                line_generator,
                secret_key,
                config_reader,
                event_handler,
            )),
            fast_mode: false,
        }
    }

    pub fn new_fast(
        line_generator: L,
        secret_key: SecretKey,
        config_reader: C,
        event_handler: E,
    ) -> Self {
        let mut daemon = Self::new(line_generator, secret_key, config_reader, event_handler);
        daemon.fast_mode = true;
        daemon
    }

    pub async fn start(&self) -> Result<(), DaemonError> {
        let config = self
            .state
            .config_reader
            .get()
            .ok_or(DaemonError::CannotReadConfig)?;

        if let Some(listener_config) = &config.listener_config {
            let router = Router::new().route("/", routing::post(|| async { "blah" }));

            let port = listener_config.custom_port.unwrap_or(7070);
            let address = format!("0.0.0.0:{}", port);

            let listener = TcpListener::bind(address)
                .await
                .map_err(|_| DaemonError::CannotBindPort(port))?;

            tokio::spawn(async {
                axum::serve(listener, router)
                    .await
                    .expect("should run indefinitely");
            });
        }

        let scheduler = JobScheduler::new()
            .await
            .map_err(|_| DaemonError::CannotStartSender)?;

        let state = Arc::clone(&self.state);
        scheduler
            .add(
                Job::new_async(
                    match self.fast_mode {
                        true => "*/5 * * * * *",
                        false => "0 * * * *",
                    },
                    move |_, _| {
                        let state = Arc::clone(&state);
                        Box::pin(async move {
                            if let Some(config) = state.config_reader.get() {
                                exchange::send_to_listeners(
                                    Arc::clone(&state.mailroom),
                                    config,
                                    Arc::clone(&state.event_handler),
                                )
                                .await;
                            }
                        })
                    },
                )
                .map_err(|_| DaemonError::CannotStartSender)?,
            )
            .await
            .map_err(|_| DaemonError::CannotStartSender)?;

        scheduler
            .start()
            .await
            .map_err(|_| DaemonError::CannotStartSender)?;

        Ok(())
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
    fn new(line_generator: L, secret_key: SecretKey, config_reader: C, event_handler: E) -> Self {
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
                secret_key,
                flatten_time,
                interval,
            ))),
            config_reader,
            event_handler: Arc::new(Mutex::new(event_handler)),
        }
    }
}

use std::{sync::Arc, time::Duration};

use archive::{DBArchive, DBError};
use axum::{Router, extract::State, response::IntoResponse, routing};
use chrono::{DateTime, Timelike, Utc};
use relay_core::{
    crypto::SecretKey,
    mailroom::{GetNextLine, Mailroom},
};
use thiserror::Error;
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
};
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{
    config::DaemonConfig,
    event::{Event, EventSender},
};

mod archive;
mod exchange;

pub const DEFAULT_LISTENING_PORT: u16 = 7070;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("cannot start db connection")]
    CannotConnectToDB,
    #[error("cannot bind port {0} (is it in use?)")]
    CannotBindPort(u16),
    #[error("cannot start sender for some reason")]
    CannotStartSender,
}

pub struct Daemon<L>
where
    L: GetNextLine,
{
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    event_sender: EventSender,
    config: Arc<RwLock<DaemonConfig>>,
    fast_mode: bool,
}

impl<L> Daemon<L>
where
    L: GetNextLine + Sync + Send + 'static,
{
    pub async fn new(
        line_generator: L,
        event_sender: EventSender,
        secret_key: SecretKey,
        db_url: &str,
        config: DaemonConfig,
    ) -> Result<Self, DaemonError> {
        let db_archive = DBArchive::new(db_url, event_sender.clone())
            .await
            .map_err(|_| DaemonError::CannotConnectToDB)?;

        let mailroom = Arc::new(Mutex::new(Mailroom::new(
            line_generator,
            db_archive,
            secret_key,
        )));

        let config = Arc::new(RwLock::new(config));

        Ok(Self {
            mailroom,
            event_sender,
            config,
            fast_mode: false,
        })
    }

    pub async fn new_fast(
        line_generator: L,
        event_sender: EventSender,
        secret_key: SecretKey,
        db_url: &str,
        config: DaemonConfig,
    ) -> Result<Self, DaemonError> {
        let flatten_time = |datetime: DateTime<Utc>| {
            datetime
                .with_second(datetime.second() / 10 * 10)
                .expect("should be able to set seconds to a multiple of 10")
                .with_nanosecond(0)
                .expect("should be able to set any utc time to nanosecond 0")
        };
        let interval = Duration::from_secs(10);

        let db_archive = DBArchive::new(db_url, event_sender.clone())
            .await
            .map_err(|_| DaemonError::CannotConnectToDB)?;

        let mailroom = Arc::new(Mutex::new(Mailroom::new_with_custom_time(
            line_generator,
            db_archive,
            secret_key,
            flatten_time,
            interval,
        )));

        let config = Arc::new(RwLock::new(config));

        Ok(Self {
            mailroom,
            event_sender,
            config,
            fast_mode: true,
        })
    }

    pub async fn start_sender(&self) -> Result<(), DaemonError> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|_| DaemonError::CannotStartSender)?;

        let mailroom = Arc::clone(&self.mailroom);
        let config = Arc::clone(&self.config);
        let event_sender = self.event_sender.clone();
        scheduler
            .add(
                Job::new_async(
                    match self.fast_mode {
                        true => "*/10 * * * * *",
                        false => "0 0 * * * *",
                    },
                    move |_, _| {
                        let mailroom = Arc::clone(&mailroom);
                        let config = Arc::clone(&config);
                        let event_sender = event_sender.clone();
                        Box::pin(async move {
                            let config = config.read().await.to_owned();
                            exchange::send_to_listeners(
                                Arc::clone(&mailroom),
                                &config,
                                event_sender.clone(),
                            )
                            .await;
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

        self.event_sender.send(Event::SenderStartedSchedule).ok();

        Ok(())
    }

    pub async fn start_listener(&self, custom_port: Option<u16>) -> Result<(), DaemonError> {
        let listener_state = Arc::new(ListenerState {
            mailroom: Arc::clone(&self.mailroom),
            event_sender: self.event_sender.clone(),
            config: Arc::clone(&self.config),
        });
        let router = Router::new()
            .route("/", routing::post(Self::handle_request))
            .with_state(listener_state);

        let port = custom_port.unwrap_or(DEFAULT_LISTENING_PORT);
        let address = format!("0.0.0.0:{}", port);

        let listener = TcpListener::bind(address)
            .await
            .map_err(|_| DaemonError::CannotBindPort(port))?;

        tokio::spawn(async {
            axum::serve(listener, router.into_make_service())
                .await
                .expect("should run indefinitely");
        });

        self.event_sender
            .send(Event::ListenerStartedListening(port))
            .ok();

        Ok(())
    }

    async fn handle_request(
        State(state): State<Arc<ListenerState<L>>>,
        body: String,
    ) -> impl IntoResponse {
        let config = &state.config.read().await.to_owned();
        exchange::respond_to_sender(
            &body,
            Arc::clone(&state.mailroom),
            config,
            state.event_sender.clone(),
        )
        .await
    }

    pub async fn update_config(&mut self, config: DaemonConfig) {
        *self.config.write().await = config;
    }
}

struct ListenerState<L: GetNextLine> {
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    event_sender: EventSender,
    config: Arc<RwLock<DaemonConfig>>,
}

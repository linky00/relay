use std::{sync::Arc, time::Duration};

use archive::{DBArchive, DBError};
use axum::{Router, extract::State, response::IntoResponse, routing};
use chrono::{DateTime, Timelike, Utc};
use relay_core::{
    crypto::SecretKey,
    mailroom::{GetNextLine, Mailroom},
};
use thiserror::Error;
use tokio::{net::TcpListener, sync::Mutex};
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
    state: Arc<Mutex<DaemonState<L>>>,
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
        Ok(Self {
            state: Arc::new(Mutex::new(
                DaemonState::new(line_generator, event_sender, secret_key, db_url, config).await?,
            )),
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
        let mut daemon =
            Self::new(line_generator, event_sender, secret_key, db_url, config).await?;
        daemon.fast_mode = true;
        Ok(daemon)
    }

    pub async fn start_sender(&self) -> Result<(), DaemonError> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|_| DaemonError::CannotStartSender)?;

        let state = Arc::clone(&self.state);
        scheduler
            .add(
                Job::new_async(
                    match self.fast_mode {
                        true => "*/10 * * * * *",
                        false => "0 * * * * *",
                    },
                    move |_, _| {
                        let state = Arc::clone(&state);
                        Box::pin(async move {
                            let state = state.lock().await;
                            exchange::send_to_listeners(
                                Arc::clone(&state.mailroom),
                                &state.config,
                                state.event_sender.clone(),
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

        self.state
            .lock()
            .await
            .event_sender
            .send(Event::SenderStartedSchedule)
            .ok();

        Ok(())
    }

    pub async fn start_listener(&self, custom_port: Option<u16>) -> Result<(), DaemonError> {
        let state = Arc::clone(&self.state);
        let router = Router::new()
            .route("/", routing::post(Self::handle_request))
            .with_state(state);

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

        self.state
            .lock()
            .await
            .event_sender
            .send(Event::ListenerStartedListening(port))
            .ok();

        Ok(())
    }

    async fn handle_request(
        State(state): State<Arc<Mutex<DaemonState<L>>>>,
        body: String,
    ) -> impl IntoResponse {
        let state = state.lock().await;
        exchange::respond_to_sender(
            &body,
            Arc::clone(&state.mailroom),
            &state.config,
            state.event_sender.clone(),
        )
        .await
    }

    pub async fn update_config(&mut self, config: DaemonConfig) {
        self.state.lock().await.config = config;
    }
}

struct DaemonState<L>
where
    L: GetNextLine,
{
    mailroom: Arc<Mutex<Mailroom<L, DBArchive, DBError>>>,
    event_sender: EventSender,
    config: DaemonConfig,
}

impl<L> DaemonState<L>
where
    L: GetNextLine + Sync + Send + 'static,
{
    async fn new(
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

        Ok(Self {
            mailroom: Arc::new(Mutex::new(Mailroom::new_with_custom_time(
                line_generator,
                db_archive,
                secret_key,
                flatten_time,
                interval,
            ))),
            event_sender,
            config,
        })
    }
}

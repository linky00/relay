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
    event::{self, Event, HandleEvent},
};

mod archive;
mod exchange;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("cannot start db connection")]
    CannotConnectToDB,
    #[error("cannot bind port {0} (is it in use?)")]
    CannotBindPort(u16),
    #[error("cannot start sender for some reason")]
    CannotStartSender,
}

pub struct Daemon<L, E>
where
    L: GetNextLine,
    E: HandleEvent + Send + 'static,
{
    state: Arc<DaemonState<L, E>>,
    fast_mode: bool,
}

impl<L, E> Daemon<L, E>
where
    L: GetNextLine + Sync + Send + 'static,
    E: HandleEvent + Sync + Send + 'static,
{
    pub async fn new(
        line_generator: L,
        event_handler: E,
        secret_key: SecretKey,
        db_url: &str,
        config: DaemonConfig,
    ) -> Result<Self, DaemonError> {
        Ok(Self {
            state: Arc::new(
                DaemonState::new(line_generator, event_handler, secret_key, db_url, config).await?,
            ),
            fast_mode: false,
        })
    }

    pub async fn new_fast(
        line_generator: L,
        event_handler: E,
        secret_key: SecretKey,
        db_url: &str,
        config: DaemonConfig,
    ) -> Result<Self, DaemonError> {
        let mut daemon =
            Self::new(line_generator, event_handler, secret_key, db_url, config).await?;
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
                        true => "*/5 * * * * *",
                        false => "0 * * * *",
                    },
                    move |_, _| {
                        let state = Arc::clone(&state);
                        Box::pin(async move {
                            exchange::send_to_listeners(
                                Arc::clone(&state.mailroom),
                                &state.config,
                                Arc::clone(&state.event_handler),
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

        event::emit_event(&self.state.event_handler, Event::SenderStartedSchedule).await;

        Ok(())
    }

    pub async fn start_listener(&self, custom_port: Option<u16>) -> Result<(), DaemonError> {
        let state = Arc::clone(&self.state);
        let router = Router::new()
            .route("/", routing::post(Self::handle_request))
            .with_state(state);

        let port = custom_port.unwrap_or(7070);
        let address = format!("0.0.0.0:{}", port);

        let listener = TcpListener::bind(address)
            .await
            .map_err(|_| DaemonError::CannotBindPort(port))?;

        tokio::spawn(async {
            axum::serve(listener, router.into_make_service())
                .await
                .expect("should run indefinitely");
        });

        event::emit_event(
            &self.state.event_handler,
            Event::ListenerStartedListening(port),
        )
        .await;

        Ok(())
    }

    async fn handle_request(
        State(state): State<Arc<DaemonState<L, E>>>,
        body: String,
    ) -> impl IntoResponse {
        exchange::respond_to_sender(
            &body,
            Arc::clone(&state.mailroom),
            &state.config,
            Arc::clone(&state.event_handler),
        )
        .await
    }
}

struct DaemonState<L, E>
where
    L: GetNextLine,
    E: HandleEvent + Send + 'static,
{
    mailroom: Arc<Mutex<Mailroom<L, DBArchive<E>, DBError>>>,
    event_handler: Arc<Mutex<E>>,
    config: DaemonConfig,
}

impl<L, E> DaemonState<L, E>
where
    L: GetNextLine + Sync + Send + 'static,
    E: HandleEvent + Sync + Send + 'static,
{
    async fn new(
        line_generator: L,
        event_handler: E,
        secret_key: SecretKey,
        db_url: &str,
        config: DaemonConfig,
    ) -> Result<Self, DaemonError> {
        let flatten_time = |datetime: DateTime<Utc>| {
            datetime
                .with_second(datetime.second() / 5 * 5)
                .expect("should be able to set seconds to a multiple of 5")
                .with_nanosecond(0)
                .expect("should be able to set any utc time to nanosecond 0")
        };
        let interval = Duration::from_secs(5);

        let event_handler = Arc::new(Mutex::new(event_handler));

        let db_archive = DBArchive::new(db_url, Arc::clone(&event_handler))
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
            event_handler,
            config,
        })
    }
}

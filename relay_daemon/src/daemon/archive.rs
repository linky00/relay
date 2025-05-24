use chrono::Utc;
use relay_core::{mailroom::Archive, message::Message};
use sqlx::{
    Error as SqlxError, Sqlite, SqlitePool,
    migrate::{MigrateDatabase, MigrateError},
};
use thiserror::Error;

use crate::event::{Event, EventSender};

#[derive(Error, Debug)]
pub(crate) enum DBError {
    #[error("cannot create db: {0}")]
    Create(#[source] SqlxError),
    #[error("cannot connect to db: {0}")]
    Connect(#[source] SqlxError),
    #[error("cannot apply migration to db: {0}")]
    Migration(#[source] MigrateError),
    #[error("db query failed: {0}")]
    Query(#[from] SqlxError),
}

pub(crate) struct DBArchive {
    pool: SqlitePool,
    event_sender: EventSender,
}

impl DBArchive {
    pub(crate) async fn new(db_url: &str, event_sender: EventSender) -> Result<Self, DBError> {
        let db_url = format!("sqlite:{db_url}");

        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            Sqlite::create_database(&db_url)
                .await
                .map_err(DBError::Create)?;
        }

        let pool = SqlitePool::connect(&db_url)
            .await
            .map_err(DBError::Connect)?;

        sqlx::migrate!()
            .run(&pool)
            .await
            .map_err(DBError::Migration)?;

        Ok(Self { pool, event_sender })
    }
}

impl Archive for DBArchive {
    type Error = DBError;

    async fn is_message_in_archive(&self, message: &Message) -> Result<bool, Self::Error> {
        Ok(sqlx::query!(
            "
            SELECT id 
            FROM messages
            WHERE signature = ?
            LIMIT 1
            ",
            message.certificate.signature
        )
        .fetch_optional(&self.pool)
        .await?
        .is_some())
    }

    async fn add_envelope_to_archive(
        &mut self,
        from: &str,
        envelope: &relay_core::message::Envelope,
    ) -> Result<(), Self::Error> {
        let timestamp = Utc::now().timestamp();

        let message_id = if let Some(found_message) = sqlx::query!(
            "
            SELECT id
            FROM messages
            WHERE signature = ?
            LIMIT 1
            ",
            envelope.message.certificate.signature
        )
        .fetch_optional(&self.pool)
        .await?
        {
            found_message.id
        } else {
            self.event_sender
                .send(Event::AddedMessageToArchive(envelope.message.clone()))
                .ok();

            sqlx::query!(
                "
                INSERT INTO messages (from_key, signature, uuid, author, line, received_at)
                VALUES (?, ?, ?, ?, ?, ?)
                ",
                envelope.message.certificate.key,
                envelope.message.certificate.signature,
                envelope.message.contents.uuid,
                envelope.message.contents.author,
                envelope.message.contents.line,
                timestamp
            )
            .execute(&self.pool)
            .await?
            .last_insert_rowid()
        };

        let envelope_id = sqlx::query!(
            "
            INSERT INTO envelopes (from_key, ttl, received_at, message_id)
            VALUES (?, ?, ?, ?)
            ",
            from,
            envelope.ttl,
            timestamp,
            message_id
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        for forwarding_key in &envelope.forwarded {
            sqlx::query!(
                "
                INSERT INTO forwards (from_key, envelope_id)
                VALUES (?, ?)
                ",
                forwarding_key,
                envelope_id
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
}

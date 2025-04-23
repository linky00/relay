use relay_core::mailroom::Archive;
use sqlx::{Sqlite, SqlitePool, migrate::MigrateDatabase};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("database operation failed")]
pub(crate) struct DBError;

pub(crate) struct DBArchive {
    pool: SqlitePool,
}

impl DBArchive {
    pub(crate) async fn new(db_url: &str) -> Result<Self, DBError> {
        if Sqlite::database_exists(db_url).await.unwrap_or(false) {
            Sqlite::create_database(db_url).await.map_err(|_| DBError)?;
        }

        let pool = SqlitePool::connect(db_url).await.map_err(|_| DBError)?;

        sqlx::migrate!().run(&pool).await.map_err(|_| DBError)?;

        Ok(Self { pool })
    }
}

impl Archive for DBArchive {
    type Error = DBError;
}

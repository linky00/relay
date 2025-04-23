use relay_core::mailroom::Archive;
use sqlx::{Sqlite, migrate::MigrateDatabase};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("database operation failed")]
pub(crate) struct DBError;

pub(crate) struct DBArchive {}

impl DBArchive {
    pub(crate) async fn new(db_url: &str) -> Result<Self, DBError> {}
}

impl Archive for DBArchive {
    type Error = DBError;
}

use std::{
    fs::{self, File},
    io::{self},
    path::Path,
};

use relay_core::crypto::SecretKey;
use thiserror::Error;

const CONFIG_FILE_PATH: &str = "relay.toml";
const POEM_FILE_PATH: &str = "poem.txt";
const LISTEN_FILE_PATH: &str = "listen.txt";
const PUBLIC_FILE_PATH: &str = "public.txt";
const SECRET_FILE_PATH: &str = "store/secret.pem";

#[derive(Error, Debug)]
pub enum FilesError {
    #[error("io error: {0}")]
    IOError(#[from] io::Error),
    #[error("trying to init in dir that is not empty")]
    InitDirNotEmpty,
    #[error("missing config file")]
    MissingConfigFile,
}

pub struct Files {
    config_file: File,
    poem_file: File,
    listen_file: File,
    secret_file: File,
}

impl Files {
    pub fn init_files(dir_path: &Path, secret_key: &SecretKey) -> Result<Self, FilesError> {
        fs::create_dir_all(dir_path)?;
        if fs::read_dir(dir_path)?.next().is_some() {
            return Err(FilesError::InitDirNotEmpty);
        };

        fs::write(
            dir_path.join(CONFIG_FILE_PATH),
            include_str!("file_templates/relay.toml"),
        );
        fs::write(
            dir_path.join(POEM_FILE_PATH),
            include_str!("file_templates/poem.txt"),
        );
        fs::write(dir_path.join(LISTEN_FILE_PATH), "");
        fs::write(
            dir_path.join(PUBLIC_FILE_PATH),
            secret_key.public_key().to_string(),
        );
        fs::write(dir_path.join(SECRET_FILE_PATH), secret_key.to_string());

        Self::open(dir_path)
    }

    pub fn open(dir_path: &Path) -> Result<Self, FilesError> {}
}

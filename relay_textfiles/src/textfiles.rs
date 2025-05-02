use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use notify::{Event, RecursiveMode, Watcher};
use pem::{Pem, PemError};
use relay_core::{
    crypto::SecretKey,
    mailroom::{DEFAULT_INITIAL_TTL, DEFAULT_MAX_FORWARDING_TTL},
};
use relay_daemon::daemon::DEFAULT_LISTENING_PORT;
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};

use crate::config::RelaytConfig;

const CONFIG_FILE_PATH: &str = "relay.toml";
const CONFIG_DEBUG_FILE_PATH: &str = "relay.debug.toml";
const POEM_FILE_PATH: &str = "poem.txt";
const LISTEN_FILE_PATH: &str = "listen.txt";
const PUBLIC_FILE_PATH: &str = "public.txt";
const ARCHIVE_FILE_PATH: &str = "store/archive.db";
const SECRET_FILE_PATH: &str = "store/secret.pem";

type WatcherReceiver = Receiver<Result<Event, notify::Error>>;

#[derive(Error, Debug)]
pub enum TextfilesError {
    #[error("io error: {0}")]
    IOError(#[from] io::Error),
    #[error("watcher error: {0}")]
    NotifyError(#[from] notify::Error),
    #[error("toml error: {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("pem error: {0}")]
    PemError(#[from] PemError),
    #[error("key is wrong length")]
    KeyLengthError,
    #[error("trying to init in dir that is not empty")]
    InitDirNotEmpty,
    #[error("missing config file")]
    MissingConfigFile,
    #[error("missing poem file")]
    MissingPoemFile,
    #[error("missing listen file")]
    MissingListenFile,
    #[error("missing secret file")]
    MissingSecretFile,
}

#[derive(Clone)]
pub struct Textfiles {
    config_path: PathBuf,
    debug_mode: bool,
    poem_path: PathBuf,
    listen_path: PathBuf,
    archive_path: PathBuf,
    secret_path: PathBuf,
}

impl Textfiles {
    pub fn new(dir_path: &Path) -> Result<Self, TextfilesError> {
        let get_existing_path = |file_path, error| {
            let full_path = dir_path.join(file_path);
            if !full_path.try_exists()? {
                return Err(error);
            }
            Ok(full_path)
        };

        let (config_path, debug_mode) =
            match get_existing_path(CONFIG_DEBUG_FILE_PATH, TextfilesError::MissingConfigFile) {
                Ok(config_path) => (config_path, true),
                Err(e) => (get_existing_path(CONFIG_FILE_PATH, e)?, false),
            };
        let poem_path = get_existing_path(POEM_FILE_PATH, TextfilesError::MissingPoemFile)?;
        let listen_path = get_existing_path(LISTEN_FILE_PATH, TextfilesError::MissingListenFile)?;
        let archive_path = dir_path.join(ARCHIVE_FILE_PATH);
        let secret_path = get_existing_path(SECRET_FILE_PATH, TextfilesError::MissingSecretFile)?;

        Ok(Textfiles {
            config_path,
            debug_mode,
            poem_path,
            listen_path,
            archive_path,
            secret_path,
        })
    }

    pub fn watch_config_changes(&self) -> Result<WatcherReceiver, TextfilesError> {
        Self::watch_file(&self.config_path)
    }

    pub fn watch_poem_changes(&self) -> Result<WatcherReceiver, TextfilesError> {
        Self::watch_file(&self.poem_path)
    }

    fn watch_file(path: &Path) -> Result<WatcherReceiver, TextfilesError> {
        let (tx, rx) = mpsc::channel(1);

        let mut watcher = notify::recommended_watcher(move |res| {
            let tx = tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(res).await;
            });
        })?;

        watcher.watch(path, RecursiveMode::Recursive)?;

        Ok(rx)
    }

    pub fn init_dir(
        dir_path: &Path,
        relay_name: &str,
        secret_key: &SecretKey,
    ) -> Result<(), TextfilesError> {
        fs::create_dir_all(dir_path)?;

        if fs::read_dir(dir_path)?.next().is_some() {
            return Err(TextfilesError::InitDirNotEmpty);
        };

        fs::create_dir(dir_path.join("store"))?;

        fs::write(
            dir_path.join(CONFIG_FILE_PATH),
            format!(
                include_str!("file_templates/relay.toml"),
                relay_name = relay_name,
                default_listening_port = DEFAULT_LISTENING_PORT,
                default_initial_ttl = DEFAULT_INITIAL_TTL,
                default_max_forwarding_ttl = DEFAULT_MAX_FORWARDING_TTL
            ),
        )?;
        fs::write(
            dir_path.join(POEM_FILE_PATH),
            include_str!("file_templates/poem.txt"),
        )?;
        fs::write(dir_path.join(LISTEN_FILE_PATH), "")?;
        fs::write(
            dir_path.join(PUBLIC_FILE_PATH),
            secret_key.public_key().to_string(),
        )?;
        fs::write(
            dir_path.join(SECRET_FILE_PATH),
            pem::encode(&Pem::new("SECRET", secret_key.as_bytes())),
        )?;

        Ok(())
    }

    pub fn read_config(&self) -> Result<RelaytConfig, TextfilesError> {
        Ok(toml::from_str(&fs::read_to_string(&self.config_path)?)?)
    }

    pub fn read_poem(&self) -> Result<Vec<String>, TextfilesError> {
        Ok(fs::read_to_string(&self.poem_path)?
            .lines()
            .map(String::from)
            .collect())
    }

    pub fn read_secret(&self) -> Result<SecretKey, TextfilesError> {
        Ok(SecretKey::new_from_bytes(
            pem::parse(fs::read_to_string(&self.secret_path)?)?
                .contents()
                .try_into()
                .map_err(|_| TextfilesError::KeyLengthError)?,
        ))
    }

    pub fn write_listen(&self, line: &str) -> Result<(), TextfilesError> {
        let mut listen_file = File::options().append(true).open(&self.listen_path)?;

        writeln!(&mut listen_file, "{line}")?;

        Ok(())
    }

    pub fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    pub fn archive_path(&self) -> &PathBuf {
        &self.archive_path
    }
}

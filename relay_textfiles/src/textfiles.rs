use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEvent, Debouncer};
use parking_lot::Mutex;
use pem::{Pem, PemError};
use relay_core::{
    crypto::SecretKey,
    mailroom::{DEFAULT_INITIAL_TTL, DEFAULT_MAX_FORWARDING_TTL},
};
use relay_daemon::daemon::DEFAULT_LISTENING_PORT;
use thiserror::Error;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use crate::config::RelaytConfig;

const CONFIG_FILE_PATH: &str = "relay.toml";
const CONFIG_DEBUG_FILE_PATH: &str = "relay.debug.toml";
const POEM_FILE_PATH: &str = "poem.txt";
const LISTEN_FILE_PATH: &str = "listen.txt";
const PUBLIC_FILE_PATH: &str = "public.txt";
const STORE_DIR_PATH: &str = "store";
const ARCHIVE_FILE_PATH: &str = "archive.db";
const SECRET_FILE_PATH: &str = "secret.pem";

type WatcherReceiver = UnboundedReceiver<Result<Vec<DebouncedEvent>, notify::Error>>;

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

#[derive(Debug, Clone)]
pub struct Textfiles {
    paths: Paths,
    watchers: Arc<Mutex<Vec<Box<Debouncer<RecommendedWatcher>>>>>,
}

impl Textfiles {
    pub fn init_regular(
        dir_path: &Path,
        relay_name: &str,
        secret_key: &SecretKey,
        debug_mode: bool,
    ) -> Result<(), TextfilesError> {
        let paths = Paths::new(dir_path, None, debug_mode);

        fs::create_dir_all(dir_path)?;
        if fs::read_dir(dir_path)?.next().is_some() {
            return Err(TextfilesError::InitDirNotEmpty);
        };

        fs::create_dir_all(dir_path.join(STORE_DIR_PATH))?;

        fs::write(
            &paths.config_path,
            format!(
                include_str!("templates/relay.toml"),
                relay_name = relay_name,
                default_listening_port = DEFAULT_LISTENING_PORT,
                default_initial_ttl = DEFAULT_INITIAL_TTL,
                default_max_forwarding_ttl = DEFAULT_MAX_FORWARDING_TTL
            ),
        )?;
        fs::write(&paths.poem_path, include_str!("templates/poem.txt"))?;

        Self::init_store_files(&paths, secret_key)?;

        Ok(())
    }

    pub fn init_store(dir_path: &Path, secret_key: &SecretKey) -> Result<(), TextfilesError> {
        let paths = Paths::new(dir_path, Some(dir_path), false);

        fs::create_dir_all(dir_path)?;
        if fs::read_dir(dir_path)?.next().is_some() {
            return Err(TextfilesError::InitDirNotEmpty);
        };

        Self::init_store_files(&paths, secret_key)?;

        Ok(())
    }

    fn init_store_files(paths: &Paths, secret_key: &SecretKey) -> Result<(), TextfilesError> {
        fs::write(&paths.listen_path, "")?;
        fs::write(&paths.public_path, secret_key.public_key().to_string())?;
        fs::write(
            &paths.secret_path,
            pem::encode(&Pem::new("SECRET", secret_key.as_bytes())),
        )?;

        Ok(())
    }

    pub fn new(
        dir_path: &Path,
        store_dir_path: Option<&Path>,
        debug_mode: bool,
    ) -> Result<Self, TextfilesError> {
        let paths = Paths::new(dir_path, store_dir_path, debug_mode);

        if !paths.config_path.exists() {
            return Err(TextfilesError::MissingConfigFile);
        }
        if !paths.poem_path.exists() {
            return Err(TextfilesError::MissingPoemFile);
        }
        if !paths.listen_path.exists() {
            return Err(TextfilesError::MissingListenFile);
        }
        if !paths.secret_path.exists() {
            return Err(TextfilesError::MissingSecretFile);
        }

        Ok(Textfiles {
            paths,
            watchers: Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn watch_config_changes(&self) -> Result<WatcherReceiver, TextfilesError> {
        self.watch_file(self.paths.config_path.clone())
    }

    pub fn watch_poem_changes(&self) -> Result<WatcherReceiver, TextfilesError> {
        self.watch_file(self.paths.poem_path.clone())
    }

    fn watch_file(&self, path: PathBuf) -> Result<WatcherReceiver, TextfilesError> {
        let (tx, rx) = mpsc::unbounded_channel();

        let mut debouncer =
            notify_debouncer_mini::new_debouncer(Duration::from_secs(1), move |event| {
                let _ = tx.send(event);
            })?;

        let watcher = debouncer.watcher();
        watcher.watch(&path, RecursiveMode::Recursive)?;

        self.watchers.lock().push(Box::new(debouncer));

        Ok(rx)
    }

    pub fn read_config(&self) -> Result<RelaytConfig, TextfilesError> {
        Ok(toml::from_str(&fs::read_to_string(
            &self.paths.config_path,
        )?)?)
    }

    pub fn read_poem(&self) -> Result<Vec<String>, TextfilesError> {
        Ok(fs::read_to_string(&self.paths.poem_path)?
            .lines()
            .map(String::from)
            .collect())
    }

    pub fn read_secret(&self) -> Result<SecretKey, TextfilesError> {
        Ok(SecretKey::new_from_bytes(
            pem::parse(fs::read_to_string(&self.paths.secret_path)?)?
                .contents()
                .try_into()
                .map_err(|_| TextfilesError::KeyLengthError)?,
        ))
    }

    pub fn write_listen(&self, line: &str) -> Result<(), TextfilesError> {
        let mut listen_file = File::options().append(true).open(&self.paths.listen_path)?;

        writeln!(&mut listen_file, "{line}")?;

        Ok(())
    }

    pub fn archive_path(&self) -> &PathBuf {
        &self.paths.archive_path
    }
}

#[derive(Debug, Clone)]
struct Paths {
    config_path: PathBuf,
    poem_path: PathBuf,
    listen_path: PathBuf,
    archive_path: PathBuf,
    public_path: PathBuf,
    secret_path: PathBuf,
}

impl Paths {
    fn new(dir_path: &Path, store_dir_path: Option<&Path>, debug_mode: bool) -> Self {
        let config_path = if debug_mode {
            dir_path.join(CONFIG_DEBUG_FILE_PATH)
        } else {
            dir_path.join(CONFIG_FILE_PATH)
        };
        let poem_path = dir_path.join(POEM_FILE_PATH);
        let (listen_path, archive_path, public_path, secret_path) =
            if let Some(store_dir_path) = store_dir_path {
                (
                    store_dir_path.join(LISTEN_FILE_PATH),
                    store_dir_path.join(ARCHIVE_FILE_PATH),
                    store_dir_path.join(PUBLIC_FILE_PATH),
                    store_dir_path.join(SECRET_FILE_PATH),
                )
            } else {
                (
                    dir_path.join(LISTEN_FILE_PATH),
                    dir_path.join(STORE_DIR_PATH).join(ARCHIVE_FILE_PATH),
                    dir_path.join(PUBLIC_FILE_PATH),
                    dir_path.join(STORE_DIR_PATH).join(SECRET_FILE_PATH),
                )
            };

        Self {
            config_path,
            poem_path,
            listen_path,
            archive_path,
            public_path,
            secret_path,
        }
    }
}

use std::{
    fs::{self},
    io::{self},
    path::{Path, PathBuf},
};

use notify::{Event, RecursiveMode, Watcher};
use pem::Pem;
use relay_core::crypto::SecretKey;
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};

const CONFIG_FILE_PATH: &str = "relay.toml";
const POEM_FILE_PATH: &str = "poem.txt";
const LISTEN_FILE_PATH: &str = "listen.txt";
const PUBLIC_FILE_PATH: &str = "public.txt";
const SECRET_FILE_PATH: &str = "store/secret.pem";

#[derive(Error, Debug)]
pub enum FilesError {
    #[error("io error: {0}")]
    IOError(#[from] io::Error),
    #[error("watcher error: {0}")]
    NotifyError(#[from] notify::Error),
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

pub struct Files {
    config_path: PathBuf,
    poem_path: PathBuf,
    listen_path: PathBuf,
    secret_path: PathBuf,
}

impl Files {
    pub fn new(dir_path: &Path) -> Result<Self, FilesError> {
        // let config_file = File::open(dir_path.join(CONFIG_FILE_PATH))
        //     .map_err(|_| FilesError::MissingConfigFile)?;
        // let poem_file =
        //     File::open(dir_path.join(POEM_FILE_PATH)).map_err(|_| FilesError::MissingPoemFile)?;
        // let listen_file = File::options()
        //     .append(true)
        //     .open(dir_path.join(LISTEN_FILE_PATH))
        //     .map_err(|_| FilesError::MissingListenFile)?;
        // let secret_file = File::open(dir_path.join(SECRET_FILE_PATH))
        //     .map_err(|_| FilesError::MissingSecretFile)?;

        // Ok(Files {
        //     config_file,
        //     poem_file,
        //     listen_file,
        //     secret_file,
        // })

        let get_existing_path = |file_path, error| {
            let full_path = dir_path.join(file_path);
            if !full_path.try_exists()? {
                return Err(error);
            }
            Ok(full_path)
        };

        let config_path = get_existing_path(CONFIG_FILE_PATH, FilesError::MissingConfigFile)?;
        let poem_path = get_existing_path(POEM_FILE_PATH, FilesError::MissingPoemFile)?;
        let listen_path = get_existing_path(LISTEN_FILE_PATH, FilesError::MissingListenFile)?;
        let secret_path = get_existing_path(SECRET_FILE_PATH, FilesError::MissingSecretFile)?;

        Ok(Files {
            config_path,
            poem_path,
            listen_path,
            secret_path,
        })
    }

    fn watch_file(path: &Path) -> notify::Result<Receiver<notify::Result<Event>>> {
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

    pub fn init_dir(dir_path: &Path, secret_key: &SecretKey) -> Result<(), FilesError> {
        fs::create_dir_all(dir_path)?;

        if fs::read_dir(dir_path)?.next().is_some() {
            return Err(FilesError::InitDirNotEmpty);
        };

        fs::create_dir(dir_path.join("store"))?;

        fs::write(
            dir_path.join(CONFIG_FILE_PATH),
            include_str!("file_templates/relay.toml"),
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
}

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use relay_core::crypto::SecretKey;

use crate::textfiles::Textfiles;

mod run;

#[derive(Parser)]
#[command(version)]
#[command(arg_required_else_help(true))]
struct RelaytCli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a relay directory
    Init {
        /// Directory to initialize
        dir: String,
        /// Optional separate storage directory to initialize
        store_dir: Option<String>,
        /// Relay name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
        /// Init with debug mode config
        #[arg(short, long)]
        debug: bool,
    },
    /// Run a relay using given directory
    Start {
        /// Relay directory
        dir: String,
        /// Optional separate storage directory
        store_dir: Option<String>,
        /// Enable debug mode
        #[arg(short, long)]
        debug: bool,
    },
}

pub async fn do_cli() -> Result<()> {
    let cli = RelaytCli::parse();

    if let Some(command) = cli.command {
        match command {
            Commands::Init {
                dir,
                store_dir,
                name,
                debug,
            } => {
                let path = Path::new(&dir);
                let store_path = store_dir.as_ref().map(|dir| Path::new(dir));
                let relay_name = name.as_deref().unwrap_or(get_relay_name_from_dir(path));
                match Textfiles::init_dir(
                    &path,
                    store_path,
                    relay_name,
                    &SecretKey::generate(),
                    debug,
                ) {
                    Ok(()) => {
                        println!("Created relay \"{relay_name}\"")
                    }
                    Err(e) => {
                        eprintln!("Could not create relay: {e}")
                    }
                }
            }
            Commands::Start {
                dir,
                store_dir,
                debug,
            } => {
                let store_path = if let Some(store_dir) = store_dir {
                    match get_checked_dir_path(&store_dir) {
                        Ok(store_path) => Some(store_path),
                        Err(_) => {
                            eprintln!("Could not open store directory \"{store_dir}\"");
                            return Ok(());
                        }
                    }
                } else {
                    None
                };
                match get_checked_dir_path(&dir) {
                    Ok(path) => {
                        println!("Starting relay \"{}\"...", get_relay_name_from_dir(&path));
                        match run::run(&path, store_path.as_deref(), debug).await {
                            Ok(()) => {}
                            Err(e) => eprintln!("Could not start relay: {e}"),
                        }
                    }
                    Err(_) => eprintln!("Could not open relay directory \"{dir}\""),
                }
            }
        }
    }

    Ok(())
}

fn get_checked_dir_path(path_string: &str) -> Result<PathBuf> {
    let path = Path::new(&path_string);
    if !path.is_dir() {
        return Err(anyhow!("can't read dir"));
    }
    Ok(path.into())
}

fn get_relay_name_from_dir(path: &Path) -> &str {
    match path.file_name() {
        Some(os_str) => os_str.try_into().unwrap_or("relay"),
        None => "relay",
    }
}

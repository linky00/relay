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
        directory: String,
    },
    /// Run a relay using given directory
    Start {
        /// Relay directory
        directory: String,
    },
}

pub async fn do_cli() -> Result<()> {
    let cli = RelaytCli::parse();

    if let Some(command) = cli.command {
        match command {
            Commands::Init { directory } => {
                let path = Path::new(&directory);
                let relay_name = get_relay_name_from_dir(path);
                match Textfiles::init_dir(&path, relay_name, &SecretKey::generate()) {
                    Ok(()) => {
                        println!("Created relay directory \"{relay_name}\"")
                    }
                    Err(e) => {
                        eprintln!("Could not create relay: {e}")
                    }
                }
            }
            Commands::Start { directory } => match get_checked_dir_path(&directory) {
                Ok(path) => {
                    println!("Starting relay \"{}\"...", get_relay_name_from_dir(&path));
                    match run::run(&path).await {
                        Ok(()) => {}
                        Err(e) => eprintln!("Could not start relay: {e}"),
                    }
                }
                Err(_) => eprintln!("Could not open relay directory \"{directory}\""),
            },
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

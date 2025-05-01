use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use relay_core::crypto::SecretKey;

use crate::{run, textfiles::Textfiles};

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
                let relay_name = path
                    .file_stem()
                    .context("coudln't get file stem")?
                    .try_into()?;
                Textfiles::init_dir(&path, relay_name, &SecretKey::generate())?;
            }
            Commands::Start { directory } => {
                let path = get_checked_dir_path(&directory).context("can't get file path")?;
                run::run(&path).await?;
            }
        }
    }

    Ok(())
}

fn get_checked_dir_path(path_string: &str) -> Result<PathBuf> {
    let path = Path::new(&path_string);
    if !path.is_dir() {
        eprintln!("\"{}\" cannot be read as a directory", path_string);
        return Err(anyhow!("can't read dir"));
    }
    Ok(path.into())
}

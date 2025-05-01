use anyhow::Result;
use relay_textfiles::cli;

#[tokio::main]
async fn main() -> Result<()> {
    cli::do_cli().await?;

    Ok(())
}

use relay_core::mailroom::{Archive, Mailroom};
use tokio::sync::Mutex;

use crate::config::Config;

pub async fn send_to_hosts<A>(
    mailroom: &Mutex<Mailroom<A>>,
    line: Option<String>,
    config: &Config,
    fast_mode: bool,
) where
    A: Archive,
{
    println!(
        "sending \"{}\" to {:#?}",
        line.unwrap_or("none".into()),
        config.contacting_hosts
    )
}

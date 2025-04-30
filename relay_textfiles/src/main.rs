use relay_textfiles::cli;

#[tokio::main]
async fn main() {
    cli::do_cli().await;
}

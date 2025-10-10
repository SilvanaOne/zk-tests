mod context;
mod url;
mod pay;
mod subscriptions;
mod users;
mod cli;

use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let cli = cli::Cli::parse();

    // Initialize tracing based on log level
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&cli.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)           // Show module path
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_ansi(true)
        )
        .init();

    // Execute CLI commands
    cli::execute_cli(cli).await
}
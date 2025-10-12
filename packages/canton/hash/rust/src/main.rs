mod add;
mod contract;
mod keccak;
mod url;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hash")]
#[command(about = "Hash contract CLI - Add integers and compute Keccak256 using Daml contract", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add an array of numbers using the Hash contract
    Add {
        /// Array of integers to add
        numbers: Vec<i64>,
    },
    /// Compute Keccak256 hash of hex-encoded integers
    Keccak {
        /// Array of integers to hash (will be converted to hex)
        numbers: Vec<i64>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Add { numbers } => add::handle_add(numbers).await?,
        Commands::Keccak { numbers } => keccak::handle_keccak(numbers).await?,
    }

    Ok(())
}

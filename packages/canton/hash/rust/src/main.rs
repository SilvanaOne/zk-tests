mod add;
mod addmapelement;
mod contract;
mod keccak;
mod merkle;
mod root;
mod sha256;
mod sha256n;
mod updatemapelement;
mod url;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hash")]
#[command(about = "Hash contract CLI - Add integers and compute hashes using Daml contract", version)]
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
    /// Compute SHA256 hash of hex-encoded integers
    Sha256 {
        /// Array of integers to hash (will be converted to hex)
        numbers: Vec<i64>,
    },
    /// Compute SHA256 hash n times iteratively
    Sha256n {
        /// Array of integers to hash (will be converted to hex)
        numbers: Vec<i64>,
        /// Number of iterations
        #[arg(short, long)]
        count: i64,
    },
    /// Calculate indexed merkle map root from key:value pairs
    Root {
        /// Array of key:value pairs (e.g., "1:20" "3:45" "67:5685")
        pairs: Vec<String>,
    },
    /// Add an element to the indexed merkle map in Daml contract
    AddMapElement {
        /// Key to insert
        key: i64,
        /// Value to insert
        value: i64,
    },
    /// Update an element in the indexed merkle map in Daml contract
    UpdateMapElement {
        /// Key to update
        key: i64,
        /// Initial value to insert
        value1: i64,
        /// New value to update to
        value2: i64,
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
        Commands::Sha256 { numbers } => sha256::handle_sha256(numbers).await?,
        Commands::Sha256n { numbers, count } => sha256n::handle_sha256n(numbers, count).await?,
        Commands::Root { pairs } => root::handle_root(pairs).await?,
        Commands::AddMapElement { key, value } => addmapelement::handle_addmapelement(key, value).await?,
        Commands::UpdateMapElement { key, value1, value2 } => updatemapelement::handle_updatemapelement(key, value1, value2).await?,
    }

    Ok(())
}

use clap::Parser;
use hash_bench::{bench, cli::Args};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sp1_sdk::utils::setup_logger();
    let report = bench::run(&Args::parse().into_config()).await?;
    report.print();
    Ok(())
}

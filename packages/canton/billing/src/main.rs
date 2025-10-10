mod cli;
mod context;
mod db;
mod metrics;
mod monitoring;
mod pay;
mod recovery;
mod subscriptions;
mod url;
mod users;

use clap::Parser;
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let cli = cli::Cli::parse();

    // Initialize logging with potential BetterStack integration
    monitoring::init_logging().await?;

    // Initialize OpenTelemetry if configured
    if let Err(e) = monitoring::init_opentelemetry().await {
        error!(error = %e, "Failed to initialize OpenTelemetry");
    }

    // Open RocksDB database
    let db_path = std::env::var("ROCKSDB_PATH").unwrap_or_else(|_| "./billing_db".to_string());
    let database = Arc::new(db::PaymentDatabase::open(&db_path)?);
    info!(path = %db_path, "Database opened successfully");

    // Initialize payment metrics
    let mut payment_metrics = metrics::PaymentMetrics::new(database.clone()).await?;
    payment_metrics.start_aggregation_task();
    info!("Payment metrics initialized");

    // Start background metrics export task
    let metrics_arc = Arc::new(payment_metrics);
    let metrics_clone = metrics_arc.clone();
    let db_clone = database.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) =
                monitoring::export_metrics_to_opentelemetry(&db_clone, &metrics_clone).await
            {
                error!(error = %e, "Failed to export metrics to OpenTelemetry");
            }
        }
    });

    // Start periodic database cleanup task
    let db_cleanup = database.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Every hour
        loop {
            interval.tick().await;
            let retention_days = std::env::var("RETENTION_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(365);
            if let Err(e) = db_cleanup.cleanup_old_data(retention_days) {
                error!(error = %e, "Failed to cleanup old data");
            }
        }
    });

    // Store database and metrics in app state for CLI commands
    let app_state = cli::AppState {
        database: database.clone(),
        metrics: metrics_arc.clone(),
    };

    // Execute CLI commands with app state
    cli::execute_cli_with_state(cli, app_state).await
}

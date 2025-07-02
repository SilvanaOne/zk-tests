use anyhow::Result;
use dotenvy;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Include the generated protobuf code
pub mod events {
    tonic::include_proto!("silvana.events");
}

// Application modules
mod buffer;
mod database;
#[path = "entity/mod.rs"]
mod entities;
mod monitoring;
mod rpc;

use buffer::EventBuffer;
use database::EventDatabase;
use events::silvana_events_service_server::SilvanaEventsServiceServer;
use monitoring::{init_monitoring, spawn_monitoring_tasks, start_metrics_server};
use rpc::SilvanaEventsServiceImpl;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize monitoring system
    init_monitoring()?;

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let server_addr = env::var("SERVER_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()
        .expect("Invalid SERVER_ADDR format");

    let metrics_addr = env::var("METRICS_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()
        .expect("Invalid METRICS_ADDR format");

    // Parse buffer configuration with memory-safe defaults
    let batch_size = env::var("BATCH_SIZE")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()
        .unwrap_or(100);

    let flush_interval_ms = env::var("FLUSH_INTERVAL_MS")
        .unwrap_or_else(|_| "500".to_string())
        .parse::<u64>()
        .unwrap_or(500);

    let channel_capacity = env::var("CHANNEL_CAPACITY")
        .unwrap_or_else(|_| "500000".to_string())
        .parse::<usize>()
        .unwrap_or(500000);

    info!("🚀 Starting Silvana RPC server");
    info!("📡 Server address: {}", server_addr);
    info!("📊 Metrics address: {}", metrics_addr);
    info!("🗄️  Database: TiDB Serverless");
    info!(
        "⚙️  Batch size: {} events (minimum trigger - actual batches may be larger)",
        batch_size
    );
    info!("⏱️  Flush interval: {}ms", flush_interval_ms);
    info!("📦 Channel capacity: {} events", channel_capacity);
    info!("🧠 Memory limit: {}MB", 100);

    // Initialize database connection
    let database = Arc::new(
        EventDatabase::new(&database_url)
            .await
            .expect("Failed to connect to database"),
    );

    info!("✅ Connected to TiDB successfully");

    // Initialize event buffer with memory-safe configuration
    let event_buffer = EventBuffer::with_config(
        Arc::clone(&database),
        batch_size,
        Duration::from_millis(flush_interval_ms),
        channel_capacity,
    );

    info!("✅ Event buffer initialized with memory safety features");

    // Start monitoring tasks
    spawn_monitoring_tasks(event_buffer.clone());

    // Create gRPC service with Prometheus metrics layer
    let events_service = SilvanaEventsServiceImpl::new(event_buffer, Arc::clone(&database));
    let grpc_service = SilvanaEventsServiceServer::new(events_service);

    info!("🎯 Starting gRPC server on {}", server_addr);

    // Start both servers concurrently
    let grpc_server = Server::builder()
        .add_service(grpc_service)
        .serve(server_addr);

    let metrics_server = start_metrics_server(metrics_addr);

    // Run both servers concurrently
    tokio::select! {
        result = grpc_server => {
            if let Err(e) = result {
                error!("gRPC server failed: {}", e);
            }
        }
        result = metrics_server => {
            if let Err(e) = result {
                error!("Metrics server failed: {}", e);
            }
        }
    }

    Ok(())
}

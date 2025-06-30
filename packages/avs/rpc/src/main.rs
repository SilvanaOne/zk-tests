use anyhow::Result;
use dotenvy;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Include the generated protobuf code
pub mod events {
    tonic::include_proto!("silvana.events");
}

// Application modules
mod buffer;
mod database;
mod entities;

use buffer::EventBuffer;
use database::EventDatabase;
use events::{
    silvana_events_service_server::{SilvanaEventsService, SilvanaEventsServiceServer},
    Event, SubmitEventsRequest, SubmitEventsResponse,
};

pub struct SilvanaEventsServiceImpl {
    event_buffer: EventBuffer,
}

impl SilvanaEventsServiceImpl {
    pub fn new(event_buffer: EventBuffer) -> Self {
        Self { event_buffer }
    }
}

#[tonic::async_trait]
impl SilvanaEventsService for SilvanaEventsServiceImpl {
    async fn submit_events(
        &self,
        request: Request<SubmitEventsRequest>,
    ) -> Result<Response<SubmitEventsResponse>, Status> {
        let events = request.into_inner().events;
        let event_count = events.len();

        debug!("Received batch of {} events", event_count);

        let mut processed_count = 0;
        let mut first_error: Option<String> = None;

        for event in events {
            match self.event_buffer.add_event(event).await {
                Ok(()) => processed_count += 1,
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(e.to_string());
                    }
                    // Continue processing other events even if one fails
                }
            }
        }

        let success = processed_count == event_count;
        let message = if success {
            format!("Successfully queued {} events", processed_count)
        } else {
            format!(
                "Queued {}/{} events. First error: {}",
                processed_count,
                event_count,
                first_error.unwrap_or_else(|| "Unknown error".to_string())
            )
        };

        if !success {
            warn!("{}", message);
        }

        Ok(Response::new(SubmitEventsResponse {
            success,
            message,
            processed_count: processed_count as u32,
        }))
    }

    async fn submit_event(
        &self,
        request: Request<Event>,
    ) -> Result<Response<SubmitEventsResponse>, Status> {
        let event = request.into_inner();

        debug!("Received single event");

        match self.event_buffer.add_event(event).await {
            Ok(()) => Ok(Response::new(SubmitEventsResponse {
                success: true,
                message: "Event queued successfully".to_string(),
                processed_count: 1,
            })),
            Err(e) => {
                warn!("Failed to queue event: {}", e);

                // Return appropriate error based on the failure type
                let status =
                    if e.to_string().contains("overloaded") || e.to_string().contains("timeout") {
                        Status::resource_exhausted(e.to_string())
                    } else if e.to_string().contains("Memory limit") {
                        Status::resource_exhausted(e.to_string())
                    } else if e.to_string().contains("Circuit breaker") {
                        Status::unavailable(e.to_string())
                    } else {
                        Status::internal(e.to_string())
                    };

                Err(status)
            }
        }
    }
}

async fn stats_reporter(buffer: EventBuffer) {
    let mut interval = interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        let stats = buffer.get_stats().await;
        let health = buffer.health_check().await;

        info!(
            "📊 Buffer Stats - Received: {}, Processed: {}, Errors: {}, Dropped: {}, Buffer: {}, Memory: {}MB, Backpressure: {}, Health: {}",
            stats.total_received,
            stats.total_processed,
            stats.total_errors,
            stats.total_dropped,
            stats.current_buffer_size,
            stats.current_memory_bytes / (1024 * 1024),
            stats.backpressure_events,
            if health { "✅" } else { "❌" }
        );

        // Alert on concerning metrics
        if stats.circuit_breaker_open {
            error!("🚨 Circuit breaker is OPEN - system overloaded!");
        }

        if stats.current_memory_bytes > 80 * 1024 * 1024 {
            // 80MB warning
            warn!(
                "⚠️  High memory usage: {}MB (80%+ of limit)",
                stats.current_memory_bytes / (1024 * 1024)
            );
        }

        if stats.total_dropped > 0 && stats.total_dropped % 100 == 0 {
            warn!("⚠️  {} events dropped due to overload", stats.total_dropped);
        }

        let backpressure_rate = if stats.total_received > 0 {
            (stats.backpressure_events as f64 / stats.total_received as f64) * 100.0
        } else {
            0.0
        };

        if backpressure_rate > 10.0 {
            warn!("⚠️  High backpressure rate: {:.1}%", backpressure_rate);
        }
    }
}

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

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let server_addr = env::var("SERVER_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()
        .expect("Invalid SERVER_ADDR format");

    // Parse buffer configuration with memory-safe defaults
    let batch_size = env::var("BATCH_SIZE")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()
        .unwrap_or(100)
        .min(1000); // Cap batch size to prevent excessive memory usage

    let flush_interval_secs = env::var("FLUSH_INTERVAL_SECS")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u64>()
        .unwrap_or(1)
        .max(1); // Minimum 1 second to prevent excessive flushing

    let channel_capacity = env::var("CHANNEL_CAPACITY")
        .unwrap_or_else(|_| "10000".to_string())
        .parse::<usize>()
        .unwrap_or(10000)
        .min(50000); // Cap channel capacity

    info!("🚀 Starting Silvana RPC server");
    info!("📡 Server address: {}", server_addr);
    info!("🗄️  Database: TiDB Serverless");
    info!(
        "⚙️  Batch size: {} events (minimum trigger - actual batches may be larger)",
        batch_size
    );
    info!("⏱️  Flush interval: {}s", flush_interval_secs);
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
        database,
        batch_size,
        Duration::from_secs(flush_interval_secs),
        channel_capacity,
    );

    info!("✅ Event buffer initialized with memory safety features");

    // Start stats reporting
    let stats_buffer = event_buffer.clone();
    tokio::spawn(async move {
        stats_reporter(stats_buffer).await;
    });

    // Start health monitoring
    let health_buffer = event_buffer.clone();
    tokio::spawn(async move {
        let mut health_interval = interval(Duration::from_secs(10));
        loop {
            health_interval.tick().await;
            let health = health_buffer.health_check().await;
            if !health {
                error!("🚨 System health check FAILED - degraded performance detected");
            }
        }
    });

    // Create gRPC service
    let events_service = SilvanaEventsServiceImpl::new(event_buffer);

    info!("🎯 Starting gRPC server on {}", server_addr);

    // Start the server
    Server::builder()
        .add_service(SilvanaEventsServiceServer::new(events_service))
        .serve(server_addr)
        .await?;

    Ok(())
}

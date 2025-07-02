use anyhow::Result;
use axum::http::{header, StatusCode};
use axum::{response::Response, routing::get, Router};
use prometheus::{
    register_int_counter, register_int_gauge, Encoder, IntCounter, IntGauge, TextEncoder,
};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::buffer::EventBuffer;

// Custom Prometheus metrics
static BUFFER_EVENTS_TOTAL: OnceLock<IntCounter> = OnceLock::new();
static BUFFER_EVENTS_PROCESSED: OnceLock<IntCounter> = OnceLock::new();
static BUFFER_EVENTS_DROPPED: OnceLock<IntCounter> = OnceLock::new();
static BUFFER_EVENTS_ERROR: OnceLock<IntCounter> = OnceLock::new();
static BUFFER_SIZE_CURRENT: OnceLock<IntGauge> = OnceLock::new();
static BUFFER_MEMORY_BYTES: OnceLock<IntGauge> = OnceLock::new();
static BUFFER_BACKPRESSURE_EVENTS: OnceLock<IntCounter> = OnceLock::new();
static BUFFER_HEALTH_STATUS: OnceLock<IntGauge> = OnceLock::new();
static CIRCUIT_BREAKER_STATUS: OnceLock<IntGauge> = OnceLock::new();

// Additional gRPC metrics
static GRPC_REQUESTS_TOTAL: OnceLock<IntCounter> = OnceLock::new();
static GRPC_REQUEST_DURATION: OnceLock<prometheus::HistogramVec> = OnceLock::new();

/// Initialize all monitoring components
pub fn init_monitoring() -> Result<()> {
    // Initialize custom application metrics
    init_custom_metrics()?;

    info!("📊 Monitoring system initialized");
    Ok(())
}

/// Initialize custom Prometheus metrics
fn init_custom_metrics() -> Result<()> {
    BUFFER_EVENTS_TOTAL
        .set(register_int_counter!(
            "silvana_buffer_events_total",
            "Total number of events received by the buffer"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_EVENTS_TOTAL"))?;

    BUFFER_EVENTS_PROCESSED
        .set(register_int_counter!(
            "silvana_buffer_events_processed_total",
            "Total number of events processed by the buffer"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_EVENTS_PROCESSED"))?;

    BUFFER_EVENTS_DROPPED
        .set(register_int_counter!(
            "silvana_buffer_events_dropped_total",
            "Total number of events dropped due to overload"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_EVENTS_DROPPED"))?;

    BUFFER_EVENTS_ERROR
        .set(register_int_counter!(
            "silvana_buffer_events_error_total",
            "Total number of events that failed processing"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_EVENTS_ERROR"))?;

    BUFFER_SIZE_CURRENT
        .set(register_int_gauge!(
            "silvana_buffer_size_current",
            "Current number of events in the buffer"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_SIZE_CURRENT"))?;

    BUFFER_MEMORY_BYTES
        .set(register_int_gauge!(
            "silvana_buffer_memory_bytes",
            "Current memory usage of the buffer in bytes"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_MEMORY_BYTES"))?;

    BUFFER_BACKPRESSURE_EVENTS
        .set(register_int_counter!(
            "silvana_buffer_backpressure_events_total",
            "Total number of backpressure events"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_BACKPRESSURE_EVENTS"))?;

    BUFFER_HEALTH_STATUS
        .set(register_int_gauge!(
            "silvana_buffer_health_status",
            "Buffer health status (1 = healthy, 0 = unhealthy)"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register BUFFER_HEALTH_STATUS"))?;

    CIRCUIT_BREAKER_STATUS
        .set(register_int_gauge!(
            "silvana_circuit_breaker_status",
            "Circuit breaker status (1 = open, 0 = closed)"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register CIRCUIT_BREAKER_STATUS"))?;

    // Initialize gRPC metrics
    GRPC_REQUESTS_TOTAL
        .set(register_int_counter!(
            "silvana_grpc_requests_total",
            "Total number of gRPC requests"
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register GRPC_REQUESTS_TOTAL"))?;

    use prometheus::{register_histogram_vec, HistogramOpts};
    GRPC_REQUEST_DURATION
        .set(register_histogram_vec!(
            HistogramOpts::new(
                "silvana_grpc_request_duration_seconds",
                "Duration of gRPC requests"
            )
            .buckets(vec![0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["method", "status"]
        )?)
        .map_err(|_| anyhow::anyhow!("Failed to register GRPC_REQUEST_DURATION"))?;

    Ok(())
}

/// Create metrics HTTP server
pub fn create_metrics_server() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

/// Start metrics HTTP server
pub async fn start_metrics_server(addr: SocketAddr) -> Result<()> {
    let app = create_metrics_server();
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("📊 Starting metrics server on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

/// Metrics endpoint handler
async fn metrics_handler() -> Result<Response<String>, StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    match encoder.encode_to_string(&metric_families) {
        Ok(metrics) => {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, encoder.format_type())
                .body(metrics)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(response)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Record gRPC request metrics (call this manually in your gRPC handlers)
pub fn record_grpc_request(method: &str, status: &str, duration_seconds: f64) {
    if let Some(counter) = GRPC_REQUESTS_TOTAL.get() {
        counter.inc();
    }

    if let Some(histogram) = GRPC_REQUEST_DURATION.get() {
        histogram
            .with_label_values(&[method, status])
            .observe(duration_seconds);
    }
}

/// Update Prometheus metrics with buffer stats
pub fn update_buffer_metrics(stats: &crate::buffer::BufferStats, health: bool) {
    if let (
        Some(events_total),
        Some(events_processed),
        Some(events_dropped),
        Some(events_error),
        Some(buffer_size),
        Some(memory_bytes),
        Some(backpressure_events),
        Some(health_status),
        Some(circuit_breaker),
    ) = (
        BUFFER_EVENTS_TOTAL.get(),
        BUFFER_EVENTS_PROCESSED.get(),
        BUFFER_EVENTS_DROPPED.get(),
        BUFFER_EVENTS_ERROR.get(),
        BUFFER_SIZE_CURRENT.get(),
        BUFFER_MEMORY_BYTES.get(),
        BUFFER_BACKPRESSURE_EVENTS.get(),
        BUFFER_HEALTH_STATUS.get(),
        CIRCUIT_BREAKER_STATUS.get(),
    ) {
        // Set counter values (Prometheus counters are monotonic, so we need to increment by the difference)
        static mut LAST_RECEIVED: u64 = 0;
        static mut LAST_PROCESSED: u64 = 0;
        static mut LAST_DROPPED: u64 = 0;
        static mut LAST_ERRORS: u64 = 0;
        static mut LAST_BACKPRESSURE: u64 = 0;

        unsafe {
            let received_diff = stats.total_received.saturating_sub(LAST_RECEIVED);
            let processed_diff = stats.total_processed.saturating_sub(LAST_PROCESSED);
            let dropped_diff = stats.total_dropped.saturating_sub(LAST_DROPPED);
            let errors_diff = stats.total_errors.saturating_sub(LAST_ERRORS);
            let backpressure_diff = stats.backpressure_events.saturating_sub(LAST_BACKPRESSURE);

            if received_diff > 0 {
                events_total.inc_by(received_diff);
                LAST_RECEIVED = stats.total_received;
            }
            if processed_diff > 0 {
                events_processed.inc_by(processed_diff);
                LAST_PROCESSED = stats.total_processed;
            }
            if dropped_diff > 0 {
                events_dropped.inc_by(dropped_diff);
                LAST_DROPPED = stats.total_dropped;
            }
            if errors_diff > 0 {
                events_error.inc_by(errors_diff);
                LAST_ERRORS = stats.total_errors;
            }
            if backpressure_diff > 0 {
                backpressure_events.inc_by(backpressure_diff);
                LAST_BACKPRESSURE = stats.backpressure_events;
            }
        }

        // Set gauge values (current state)
        buffer_size.set(stats.current_buffer_size as i64);
        memory_bytes.set(stats.current_memory_bytes as i64);
        health_status.set(if health { 1 } else { 0 });
        circuit_breaker.set(if stats.circuit_breaker_open { 1 } else { 0 });
    }
}

/// Runs periodic statistics reporting for the event buffer
pub async fn stats_reporter(buffer: EventBuffer) {
    let mut interval = interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        let stats = buffer.get_stats().await;
        let health = buffer.health_check().await;

        // Update Prometheus metrics
        update_buffer_metrics(&stats, health);

        if !health {
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
        }

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

/// Runs periodic health monitoring for the event buffer
pub async fn health_monitor(buffer: EventBuffer) {
    let mut health_interval = interval(Duration::from_secs(10));

    loop {
        health_interval.tick().await;
        let health = buffer.health_check().await;
        if !health {
            error!("🚨 System health check FAILED - degraded performance detected");
        }
    }
}

/// Spawns both stats reporting and health monitoring tasks
pub fn spawn_monitoring_tasks(buffer: EventBuffer) {
    // Start stats reporting
    let stats_buffer = buffer.clone();
    tokio::spawn(async move {
        stats_reporter(stats_buffer).await;
    });

    // Start health monitoring
    let health_buffer = buffer.clone();
    tokio::spawn(async move {
        health_monitor(health_buffer).await;
    });
}

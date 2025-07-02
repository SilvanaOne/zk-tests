use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::buffer::EventBuffer;

/// Runs periodic statistics reporting for the event buffer
pub async fn stats_reporter(buffer: EventBuffer) {
    let mut interval = interval(Duration::from_secs(30));

    loop {
        interval.tick().await;

        let stats = buffer.get_stats().await;
        let health = buffer.health_check().await;

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

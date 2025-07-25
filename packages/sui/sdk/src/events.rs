use crate::constants::{TARGET_MODULE, TARGET_PACKAGE_ID};
use anyhow::Result;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU64, Ordering};
use sui_rpc::Client;
use sui_rpc::proto::sui::rpc::v2beta2::{
    SubscribeCheckpointsRequest, SubscribeCheckpointsResponse,
};
use tokio::time::{sleep, timeout};
use tokio_stream::StreamExt;
use tonic::Request;
use prost::Message;
use prost_types;

async fn create_checkpoint_stream(
    client: &mut Client,
) -> Result<impl tokio_stream::Stream<Item = Result<SubscribeCheckpointsResponse, tonic::Status>>> {
    let mut subscription_client = client.subscription_client();

    let request = Request::new(SubscribeCheckpointsRequest {
        read_mask: Some(prost_types::FieldMask {
            paths: vec![
                "summary.timestamp".to_string(), // For freshness checking
                "transactions.events.events.package_id".to_string(), // For package filtering
                "transactions.events.events.module".to_string(), // For display
                                                 //"transactions.events.events.sender".to_string(), // For display
                                                 //"transactions.events.events.event_type".to_string(), // For display
            ],
        }),
    });

    let stream = subscription_client
        .subscribe_checkpoints(request)
        .await?
        .into_inner();

    Ok(stream)
}

pub async fn stream_until_target_events(
    client: &mut Client,
    target_event_count: usize,
) -> Result<()> {
    let mut count = 0;
    let mut total_target_events = 0;
    let mut total_delay_ms = 0.0;
    let mut retry_count = 0;
    let mut total_bytes_received = 0u64;
    let mut total_checkpoints_with_events = 0;
    let start_time = SystemTime::now();
    const MAX_RETRIES: usize = 5;
    const TIMEOUT_SECONDS: u64 = 30;
    const LOG_INTERVAL_SECONDS: u64 = 600; // 10 minutes
    
    static LAST_LOG_TIME: AtomicU64 = AtomicU64::new(0);

    println!("Starting to stream checkpoints...");
    println!(
        "Looking for events with package_id = {} and module = {}",
        TARGET_PACKAGE_ID, TARGET_MODULE
    );
    println!("Target: {} events\n", target_event_count);

    loop {
        match create_checkpoint_stream(client).await {
            Ok(mut stream) => {
                retry_count = 0; // Reset retry count on successful connection

                loop {
                    match timeout(Duration::from_secs(TIMEOUT_SECONDS), stream.next()).await {
                        Ok(Some(response)) => match response {
                            Ok(response) => {
                                // Calculate response size
                                let response_size = response.encoded_len() as u64;
                                total_bytes_received += response_size;
                                
                                let SubscribeCheckpointsResponse { cursor, checkpoint } = response;
                                count += 1;
                                let mut checkpoint_target_events = 0;
                                let mut cursor_value = 0u64;
                                let mut delay_ms = 0.0;

                                if let Some(cursor) = cursor {
                                    cursor_value = cursor;
                                }

                                if let Some(checkpoint) = checkpoint {
                                    let current_time_ms = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis()
                                        as f64;

                                    let checkpoint_timestamp_ms;

                                    if let Some(summary) = &checkpoint.summary {
                                        if let Some(timestamp) = &summary.timestamp {
                                            checkpoint_timestamp_ms = (timestamp.seconds as f64
                                                * 1000.0)
                                                + (timestamp.nanos as f64 / 1_000_000.0);
                                            delay_ms = current_time_ms - checkpoint_timestamp_ms;
                                        }
                                    }

                                    // Count events matching our target package and module
                                    for transaction in &checkpoint.transactions {
                                        if let Some(events) = &transaction.events {
                                            for event in &events.events {
                                                if let (Some(package_id), Some(module)) =
                                                    (&event.package_id, &event.module)
                                                {
                                                    if package_id == TARGET_PACKAGE_ID
                                                        && module == TARGET_MODULE
                                                    {
                                                        checkpoint_target_events += 1;
                                                        total_target_events += 1;
                                                        total_delay_ms += delay_ms;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Rate-limited logging for checkpoint info
                                    let current_time_secs = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    
                                    let last_log_time = LAST_LOG_TIME.load(Ordering::Relaxed);
                                    let should_log = last_log_time == 0 || 
                                        current_time_secs.saturating_sub(last_log_time) >= LOG_INTERVAL_SECONDS;
                                    
                                    if checkpoint_target_events > 0 {
                                        total_checkpoints_with_events += 1;
                                        
                                        if should_log {
                                            LAST_LOG_TIME.store(current_time_secs, Ordering::Relaxed);
                                            println!(
                                                "Checkpoint #{}: Cursor={}, Events={}, Delay={:.1}ms, Size={}B, Total={}",
                                                count,
                                                cursor_value,
                                                checkpoint_target_events,
                                                delay_ms,
                                                response_size,
                                                total_target_events
                                            );
                                        }
                                    } else if should_log {
                                        LAST_LOG_TIME.store(current_time_secs, Ordering::Relaxed);
                                        println!(
                                            "Checkpoint #{}: Cursor={}, Events=0, Delay={:.1}ms, Size={}B",
                                            count,
                                            cursor_value,
                                            delay_ms,
                                            response_size
                                        );
                                    }
                                }

                                if total_target_events >= target_event_count {
                                    let elapsed_time = start_time.elapsed().unwrap_or_default();
                                    let elapsed_secs = elapsed_time.as_secs_f64();
                                    
                                    println!("\n=== SUMMARY ===");
                                    println!("Received {} checkpoints total", count);
                                    println!("Checkpoints with target events: {}", total_checkpoints_with_events);
                                    println!(
                                        "Total target events found: {} (target: {})",
                                        total_target_events, target_event_count
                                    );

                                    if total_target_events > 0 {
                                        let average_delay_ms =
                                            total_delay_ms / total_target_events as f64;
                                        println!(
                                            "Average delay for target events: {:.1}ms",
                                            average_delay_ms
                                        );
                                    }
                                    
                                    // Network statistics
                                    println!("\n=== NETWORK STATISTICS ===");
                                    println!("Total bytes received: {} ({:.2} KB, {:.2} MB)", 
                                            total_bytes_received, 
                                            total_bytes_received as f64 / 1024.0,
                                            total_bytes_received as f64 / (1024.0 * 1024.0));
                                    println!("Average bytes per checkpoint: {:.1}", 
                                            total_bytes_received as f64 / count as f64);
                                    if total_checkpoints_with_events > 0 {
                                        println!("Average bytes per checkpoint with events: {:.1}", 
                                                total_bytes_received as f64 / total_checkpoints_with_events as f64);
                                    }
                                    println!("Bytes per target event: {:.1}", 
                                            total_bytes_received as f64 / total_target_events as f64);
                                    println!("Duration: {:.2}s", elapsed_secs);
                                    if elapsed_secs > 0.0 {
                                        println!("Data rate: {:.2} KB/s", 
                                                (total_bytes_received as f64 / 1024.0) / elapsed_secs);
                                        println!("Checkpoints per second: {:.2}", 
                                                count as f64 / elapsed_secs);
                                        println!("Events per second: {:.2}", 
                                                total_target_events as f64 / elapsed_secs);
                                    }

                                    println!(
                                        "\nEvents matching package {} and module {}",
                                        TARGET_PACKAGE_ID, TARGET_MODULE
                                    );
                                    return Ok(());
                                }
                            }
                            Err(e) => {
                                eprintln!("Stream error: {}", e);
                                break; // Break inner loop to retry connection
                            }
                        },
                        Ok(None) => {
                            println!("Stream ended unexpectedly. Attempting to reconnect...");
                            break; // Break inner loop to retry connection
                        }
                        Err(_) => {
                            println!(
                                "Stream timeout after {} seconds. Attempting to reconnect...",
                                TIMEOUT_SECONDS
                            );
                            break; // Break inner loop to retry connection
                        }
                    }
                }
            }
            Err(e) => {
                retry_count += 1;
                if retry_count > MAX_RETRIES {
                    eprintln!("Failed to connect after {} attempts: {}", MAX_RETRIES, e);
                    return Err(e);
                }

                let delay_secs = 2u64.pow(retry_count as u32).min(60); // Exponential backoff, max 60 seconds
                eprintln!(
                    "Connection failed (attempt {}/{}): {}. Retrying in {} seconds...",
                    retry_count, MAX_RETRIES, e, delay_secs
                );
                sleep(Duration::from_secs(delay_secs)).await;
            }
        }
    }
}

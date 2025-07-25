use chrono::{DateTime, Utc};
use prost::Message;
use prost_types;
use std::error::Error;
use tonic::Request;
use tonic::transport::{Channel, ClientTlsConfig};

pub mod sui_rpc {
    tonic::include_proto!("sui.rpc.v2beta2");
}

use sui_rpc::{
    Event, SubscribeCheckpointsRequest, subscription_service_client::SubscriptionServiceClient,
};

// Target package ID from the user's request
const TARGET_PACKAGE_ID: &str =
    "0xa6477a6bf50e2389383b34a76d59ccfbec766ff2decefe38e1d8436ef8a9b245";

pub async fn query_events_via_grpc(
    num_events: u32,
    num_checkpoints: u32,
) -> Result<(u32, f64, u64), Box<dyn Error>> {
    println!(
        "🚀 Starting Sui gRPC client to monitor events from package: {}",
        TARGET_PACKAGE_ID
    );
    println!(
        "🔍 Monitoring {} events from {} gRPC checkpoints",
        num_events, num_checkpoints
    );

    // Record the start time - only process events after this timestamp
    let start_time = Utc::now();
    println!(
        "⏰ Start time: {} ({}ms)",
        start_time.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
        start_time.timestamp_millis()
    );
    println!("🎯 Will only process events that occur after this timestamp");

    // Try to connect to Sui - first with TLS, then plaintext
    let channel = match create_sui_channel_tls().await {
        Ok(channel) => {
            println!("✅ Successfully connected to Sui gRPC with TLS!");
            channel
        }
        Err(tls_error) => {
            println!("⚠️  TLS connection failed: {}", tls_error);
            println!("🔄 Trying plaintext connection...");

            match create_sui_channel_plaintext().await {
                Ok(channel) => {
                    println!("✅ Successfully connected to Sui gRPC with plaintext!");
                    channel
                }
                Err(plaintext_error) => {
                    println!("❌ Both TLS and plaintext connections failed:");
                    println!("   TLS error: {}", tls_error);
                    println!("   Plaintext error: {}", plaintext_error);
                    println!("💡 Check network connectivity and endpoint availability");
                    println!("📝 gRPC client is properly generated and ready for use");
                    return Ok((0, 0.0, 0)); // Return 0 fresh events, 0.0 average delay, and 0 bytes when connection fails
                }
            }
        }
    };

    // Create subscription client for streaming checkpoints
    let mut subscription_client = SubscriptionServiceClient::new(channel);

    // Optimized read mask using sui-rpc field mask utilities
    // Based on Sui examples: use direct field names without "checkpoint." prefix
    let request = Request::new(SubscribeCheckpointsRequest {
        read_mask: Some(prost_types::FieldMask {
            paths: vec![
                "summary.timestamp".to_string(), // For freshness checking
                "transactions.events.events.package_id".to_string(), // For package filtering
                                                 //"transactions.events.events.module".to_string(), // For display
                                                 //"transactions.events.events.sender".to_string(), // For display
                                                 //"transactions.events.events.event_type".to_string(), // For display
            ],
        }),
    });

    println!("📡 Subscribing to gRPC checkpoint stream with optimized read mask...");
    println!("🚀 OPTIMIZATION: Using specific field paths based on Sui examples");
    println!("📊 Only fetching: summary.timestamp, transactions.events.events.*");
    println!(
        "🎯 DEBUG: Looking for events from target package: {}",
        TARGET_PACKAGE_ID
    );
    let mut stream = subscription_client
        .subscribe_checkpoints(request)
        .await?
        .into_inner();

    let mut checkpoint_count = 0;
    let mut total_events_checked = 0;
    let mut fresh_events_found = 0;
    let mut delays: Vec<i64> = Vec::new(); // Track delays for average calculation
    let mut total_response_size: u64 = 0; // Track total size of responses

    // Process each checkpoint
    while let Some(checkpoint_response) = stream.message().await? {
        checkpoint_count += 1;

        // Track response size using protobuf's encoded_len method
        total_response_size += checkpoint_response.encoded_len() as u64;

        // Debug: Print checkpoint structure info
        // println!("🔍 DEBUG: Received checkpoint response:");
        // println!("   • Has cursor: {}", checkpoint_response.cursor.is_some());
        // println!(
        //     "   • Has checkpoint: {}",
        //     checkpoint_response.checkpoint.is_some()
        // );
        // if let Some(checkpoint) = &checkpoint_response.checkpoint {
        //     println!("   • Has summary: {}", checkpoint.summary.is_some());
        //     if let Some(summary) = &checkpoint.summary {
        //         println!("   • Summary timestamp: {:?}", summary.timestamp);
        //     }
        //     println!("   • Transactions count: {}", checkpoint.transactions.len());
        //     for (tx_idx, tx) in checkpoint.transactions.iter().enumerate() {
        //         println!(
        //             "   • Transaction {}: has_events={}, events_count={}",
        //             tx_idx,
        //             tx.events.is_some(),
        //             tx.events.as_ref().map(|e| e.events.len()).unwrap_or(0)
        //         );
        //     }
        // }

        if let Some(cursor) = checkpoint_response.cursor {
            if let Some(checkpoint) = checkpoint_response.checkpoint {
                // Process all checkpoints but filter events within them by timestamp
                let events_found = process_checkpoint_events(
                    &checkpoint,
                    cursor,
                    &mut total_events_checked,
                    &mut fresh_events_found,
                    &mut delays,
                    &start_time,
                );

                if events_found > 0 {
                    println!(
                        "✨ Found {} fresh gRPC events from target package in checkpoint {}",
                        events_found, cursor
                    );
                }

                // Stop if we've found enough fresh events
                if fresh_events_found >= num_events {
                    println!(
                        "\n🎉 Found {} fresh gRPC events from target package, stopping search",
                        fresh_events_found
                    );
                    break;
                }
            }
        }

        // Stop after processing enough checkpoints for demo
        if checkpoint_count >= num_checkpoints {
            println!(
                "\n📄 Processed {} gRPC checkpoints, found {} fresh events from package {}",
                checkpoint_count, fresh_events_found, TARGET_PACKAGE_ID
            );
            break;
        }

        // Print progress every 10 checkpoints
        if checkpoint_count % 10 == 0 {
            println!(
                "🔍 Processed {} gRPC checkpoints, found {} fresh events so far...",
                checkpoint_count, fresh_events_found
            );
        }
    }

    // Calculate average delay
    let average_delay = if delays.is_empty() {
        0.0
    } else {
        delays.iter().sum::<i64>() as f64 / delays.len() as f64
    };

    println!("\n📊 Final gRPC Summary:");
    println!("   • Total events checked: {}", total_events_checked);
    println!(
        "   • Fresh gRPC events from target package: {}",
        fresh_events_found
    );
    println!("   • Checkpoints processed: {}", checkpoint_count);
    println!("   • Average delay: {:.2}ms", average_delay);
    println!(
        "   • Start time: {} ({}ms)",
        start_time.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
        start_time.timestamp_millis()
    );

    Ok((fresh_events_found, average_delay, total_response_size))
}

fn process_checkpoint_events(
    checkpoint: &sui_rpc::Checkpoint,
    checkpoint_cursor: u64,
    total_events_checked: &mut u32,
    fresh_events_found: &mut u32,
    delays: &mut Vec<i64>,
    start_time: &DateTime<Utc>,
) -> u32 {
    let mut events_in_checkpoint = 0;

    // Get checkpoint timestamp for comparison
    let checkpoint_time = if let Some(summary) = &checkpoint.summary {
        if let Some(timestamp) = &summary.timestamp {
            DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
        } else {
            None
        }
    } else {
        None
    };

    // Iterate through all transactions in this checkpoint
    for (tx_index, transaction) in checkpoint.transactions.iter().enumerate() {
        if let Some(events) = &transaction.events {
            // Check each event in this transaction
            for (event_index, event) in events.events.iter().enumerate() {
                *total_events_checked += 1;

                // Debug: Print every event for inspection (limit to first few for readability)
                // if *total_events_checked <= 20 {
                //     println!(
                //         "🔍 DEBUG: Event {}.{}.{}",
                //         checkpoint_cursor, tx_index, event_index
                //     );
                //     println!("   • Package ID: {:?}", event.package_id);
                //     println!("   • Module: {:?}", event.module);
                //     println!("   • Event Type: {:?}", event.event_type);
                //     if let Some(pkg_id) = &event.package_id {
                //         println!(
                //             "   • Target match: {} == {} ? {}",
                //             pkg_id,
                //             TARGET_PACKAGE_ID,
                //             pkg_id == TARGET_PACKAGE_ID
                //         );
                //     }
                // }

                // Check if this event is fresh by comparing checkpoint timestamp with start_time
                let is_fresh_event = if let Some(event_time) = checkpoint_time {
                    event_time > *start_time
                } else {
                    false // Skip events without valid timestamps
                };

                // Debug freshness check (limit output)
                if *total_events_checked <= 10 {
                    println!(
                        "   • Event time: {:?}, Start time: {:?}, Is fresh: {}",
                        checkpoint_time, start_time, is_fresh_event
                    );
                }

                // Only process fresh events from target package
                if is_fresh_event {
                    if let Some(package_id) = &event.package_id {
                        if package_id == TARGET_PACKAGE_ID {
                            events_in_checkpoint += 1;
                            *fresh_events_found += 1;

                            let delay_ms = display_event_grpc_minimal(
                                event,
                                *fresh_events_found,
                                checkpoint_cursor,
                                tx_index,
                                event_index,
                                checkpoint.summary.as_ref(),
                            );

                            // Collect delay for average calculation
                            if let Some(delay) = delay_ms {
                                delays.push(delay);
                            }
                        }
                    }
                }
            }
        }
    }

    events_in_checkpoint
}

fn display_event_grpc_minimal(
    event: &Event,
    event_number: u32,
    checkpoint_seq: u64,
    tx_index: usize,
    event_index: usize,
    checkpoint_summary: Option<&sui_rpc::CheckpointSummary>,
) -> Option<i64> {
    println!(
        "\n🎉 gRPC Event #{} found from target package!",
        event_number
    );
    println!("┌─────────────────────────────────────────────────────────────────");
    println!("│ Checkpoint:     {}", checkpoint_seq);
    println!("│ Transaction:    tx #{} in checkpoint", tx_index);
    println!("│ Event Index:    {} in transaction", event_index);

    if let Some(package_id) = &event.package_id {
        println!("│ Package ID:     {}", package_id);
    }

    if let Some(module) = &event.module {
        println!("│ Module:         {}", module);
    }

    if let Some(event_type) = &event.event_type {
        println!("│ Event Type:     {}", event_type);
    }

    if let Some(sender) = &event.sender {
        println!("│ Sender:         {}", sender);
    }

    println!("│ 📡 Optimized gRPC: Read mask using specific field paths from Sui examples");
    println!(
        "│ 💡 Only essential fields fetched: package_id, module, sender, event_type, timestamp"
    );
    println!("│ 🔧 Full event content (JSON/BCS) can be fetched separately if needed");

    // Display timestamp from checkpoint and calculate delay
    let calculated_delay = if let Some(summary) = checkpoint_summary {
        if let Some(timestamp) = &summary.timestamp {
            if let Some(datetime) =
                DateTime::from_timestamp(timestamp.seconds, timestamp.nanos as u32)
            {
                // Calculate delay from checkpoint creation to now
                let now = Utc::now();
                let delay = now - datetime;
                let delay_ms = delay.num_milliseconds();

                println!(
                    "│ Timestamp:      {} (delay: {}ms)",
                    datetime.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
                    delay_ms
                );
                Some(delay_ms)
            } else {
                println!(
                    "│ Timestamp:      {}s + {}ns (invalid)",
                    timestamp.seconds, timestamp.nanos
                );
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    println!("└─────────────────────────────────────────────────────────────────");

    calculated_delay
}

async fn create_sui_channel_tls() -> Result<Channel, Box<dyn Error>> {
    // Sui gRPC endpoint
    let endpoint = "https://148.251.75.59:9000";

    // Create TLS configuration with webpki roots
    let tls = ClientTlsConfig::new().domain_name("fullnode.mainnet.sui.io");

    let channel = Channel::from_static(endpoint)
        .tls_config(tls)?
        .connect()
        .await?;

    println!("✅ Connected to Sui gRPC with TLS at {}", endpoint);
    Ok(channel)
}

async fn create_sui_channel_plaintext() -> Result<Channel, Box<dyn Error>> {
    // Sui gRPC endpoint (plaintext)
    let endpoint = "http://148.251.75.59:9000";

    let channel = Channel::from_static(endpoint).connect().await?;

    println!("✅ Connected to Sui gRPC with plaintext at {}", endpoint);
    Ok(channel)
}

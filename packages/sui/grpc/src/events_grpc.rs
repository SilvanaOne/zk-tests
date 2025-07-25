use chrono::{DateTime, Utc};
use prost::Message;
use prost_types;
use serde_json;
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
        "🔍 Monitoring {} events from {} checkpoints",
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

    // Subscribe to checkpoint stream
    let request = Request::new(SubscribeCheckpointsRequest {
        read_mask: Some(prost_types::FieldMask {
            paths: vec![
                "sequence_number".to_string(),
                "digest".to_string(),
                "summary".to_string(),
                "transactions".to_string(),
            ],
        }),
    });

    println!("📡 Subscribing to checkpoint stream to monitor fresh events...");
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
                        "✨ Found {} fresh events from target package in checkpoint {}",
                        events_found, cursor
                    );
                }

                // Stop if we've found enough fresh events
                if fresh_events_found >= num_events {
                    println!(
                        "\n🎉 Found {} fresh events from target package, stopping search",
                        fresh_events_found
                    );
                    break;
                }
            }
        }

        // Stop after processing enough checkpoints for demo
        if checkpoint_count >= num_checkpoints {
            println!(
                "\n📄 Processed {} checkpoints, found {} fresh events from package {}",
                checkpoint_count, fresh_events_found, TARGET_PACKAGE_ID
            );
            break;
        }

        // Print progress every 10 checkpoints
        if checkpoint_count % 10 == 0 {
            println!(
                "🔍 Processed {} checkpoints, found {} fresh events so far...",
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

    println!("\n📊 Final Summary:");
    println!("   • Total events checked: {}", total_events_checked);
    println!(
        "   • Fresh events from target package: {}",
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

                // Check if this event is fresh by comparing checkpoint timestamp with start_time
                let is_fresh_event = if let Some(event_time) = checkpoint_time {
                    event_time > *start_time
                } else {
                    false // Skip events without valid timestamps
                };

                // Only process fresh events from target package
                if is_fresh_event {
                    if let Some(package_id) = &event.package_id {
                        if package_id == TARGET_PACKAGE_ID {
                            events_in_checkpoint += 1;
                            *fresh_events_found += 1;

                            let delay_ms = display_event_grpc(
                                event,
                                *fresh_events_found,
                                checkpoint_cursor,
                                tx_index,
                                event_index,
                                &transaction
                                    .digest
                                    .as_ref()
                                    .unwrap_or(&"unknown".to_string()),
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

fn display_event_grpc(
    event: &Event,
    event_number: u32,
    checkpoint_seq: u64,
    tx_index: usize,
    event_index: usize,
    tx_digest: &str,
    checkpoint_summary: Option<&sui_rpc::CheckpointSummary>,
) -> Option<i64> {
    println!("\n🎉 Event #{} found from target package!", event_number);
    println!("┌─────────────────────────────────────────────────────────────────");
    println!("│ Checkpoint:     {}", checkpoint_seq);
    println!(
        "│ Transaction:    {} (tx #{} in checkpoint)",
        tx_digest, tx_index
    );
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

    // Display JSON content if available
    if let Some(json_content) = &event.json {
        println!("│ JSON Data:");
        // Convert prost_types::Value to serde_json::Value for serialization
        match convert_prost_value_to_serde(json_content) {
            Ok(serde_value) => {
                let json_str = serde_json::to_string_pretty(&serde_value)
                    .unwrap_or_else(|_| "Failed to serialize JSON".to_string());
                for line in json_str.lines() {
                    println!("│   {}", line);
                }
            }
            Err(_) => {
                println!("│   Failed to convert JSON data");
            }
        }
    }

    // Display BCS content info
    if let Some(contents) = &event.contents {
        if let Some(bcs_bytes) = &contents.value {
            println!("│ BCS Length:     {} bytes", bcs_bytes.len());
        }
        if let Some(bcs_name) = &contents.name {
            println!("│ BCS Type:       {}", bcs_name);
        }
    }

    // Display links to explorers
    println!(
        "│ 🌐 View on Suiscan: https://suiscan.xyz/testnet/tx/{}",
        tx_digest
    );
    println!(
        "│ 📦 Checkpoint:      https://suiscan.xyz/testnet/checkpoint/{}",
        checkpoint_seq
    );
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

// Helper function to convert prost_types::Value to serde_json::Value
fn convert_prost_value_to_serde(
    prost_value: &prost_types::Value,
) -> Result<serde_json::Value, Box<dyn Error>> {
    use prost_types::value::Kind;
    use serde_json::Value as JsonValue;

    let result = match &prost_value.kind {
        Some(Kind::NullValue(_)) => JsonValue::Null,
        Some(Kind::NumberValue(n)) => JsonValue::from(*n),
        Some(Kind::StringValue(s)) => JsonValue::String(s.clone()),
        Some(Kind::BoolValue(b)) => JsonValue::Bool(*b),
        Some(Kind::StructValue(s)) => {
            let mut map = serde_json::Map::new();
            for (key, value) in &s.fields {
                map.insert(key.clone(), convert_prost_value_to_serde(value)?);
            }
            JsonValue::Object(map)
        }
        Some(Kind::ListValue(l)) => {
            let mut vec = Vec::new();
            for value in &l.values {
                vec.push(convert_prost_value_to_serde(value)?);
            }
            JsonValue::Array(vec)
        }
        None => JsonValue::Null,
    };

    Ok(result)
}

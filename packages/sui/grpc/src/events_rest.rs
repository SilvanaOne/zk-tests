use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;

// Target package ID from the user's request
#[allow(dead_code)]
const TARGET_PACKAGE_ID: &str =
    "0xa6477a6bf50e2389383b34a76d59ccfbec766ff2decefe38e1d8436ef8a9b245";

#[derive(Debug, Serialize, Deserialize)]
struct SuiEvent {
    pub id: EventId,
    #[serde(rename = "packageId")]
    pub package_id: String,
    #[serde(rename = "transactionModule")]
    pub transaction_module: String,
    pub sender: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(rename = "parsedJson")]
    pub parsed_json: Option<Value>,
    pub bcs: String,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventId {
    #[serde(rename = "txDigest")]
    pub tx_digest: String,
    #[serde(rename = "eventSeq")]
    pub event_seq: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryEventsResponse {
    pub data: Vec<SuiEvent>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<Value>,
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}
#[allow(dead_code)]
pub async fn query_events_via_rest(
    num_events: u32,
) -> Result<(u32, f64, u64), Box<dyn std::error::Error>> {
    println!(
        "🚀 Starting Sui JSON-RPC client with server-side filtering for package: {}",
        TARGET_PACKAGE_ID
    );

    let client = reqwest::Client::new();
    let sui_rpc_url = "http://148.251.75.59:9000";

    println!("✅ Connected to Sui testnet at {}", sui_rpc_url);

    // Record the start time - only process events after this timestamp
    let start_time = Utc::now();
    let start_timestamp_ms = start_time.timestamp_millis();
    println!(
        "⏰ Start time: {} ({}ms)",
        start_time.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
        start_timestamp_ms
    );
    println!("🎯 Will only process events that occur after this timestamp");
    println!("🔍 Server-side filtering: only events from target package will be returned");

    // Query recent events and filter for fresh events only
    let mut cursor: Option<Value> = None;
    let mut total_events_checked = 0;
    let mut fresh_events_found = 0;
    let mut target_events_processed = 0;
    let mut delays: Vec<i64> = Vec::new(); // Track delays for average calculation
    let mut total_response_size: u64 = 0; // Track total size of responses
    let polling_interval = Duration::from_millis(100); // Poll every 100ms
    let max_polls = 100000; // Maximum number of polling cycles (10 minutes at 100ms intervals)
    let mut poll_count = 0;

    println!("\n🔄 Starting continuous polling for fresh events...");

    loop {
        if poll_count >= max_polls {
            println!(
                "\n⏰ Reached maximum polling time ({}), stopping search",
                max_polls
            );
            break;
        }

        poll_count += 1;
        println!("\n🔍 Polling for fresh events (cycle {})...", poll_count);

        let (response, response_size) =
            query_recent_events_with_size(&client, sui_rpc_url, cursor.as_ref()).await?;
        total_response_size += response_size;

        if response.data.is_empty() {
            println!("📭 No events found in this cycle");
            sleep(polling_interval).await;
            continue;
        }

        let mut fresh_events_in_batch = 0;

        // Process events and filter for fresh ones
        for event in &response.data {
            total_events_checked += 1;

            // Check if event is fresh (occurred after start time)
            let is_fresh = if let Some(timestamp_str) = &event.timestamp_ms {
                if let Ok(event_timestamp_ms) = timestamp_str.parse::<i64>() {
                    event_timestamp_ms > start_timestamp_ms
                } else {
                    false // Skip events with invalid timestamps
                }
            } else {
                false // Skip events without timestamps
            };

            if is_fresh {
                fresh_events_in_batch += 1;
                fresh_events_found += 1;

                // All events are now from our target package (server-side filtered)
                target_events_processed += 1;
                let delay_ms = display_event(event, target_events_processed);

                // Collect delay for average calculation
                if let Some(delay) = delay_ms {
                    delays.push(delay);
                }
            }
        }

        if fresh_events_in_batch > 0 {
            println!(
                "✨ Found {} fresh events from target package in this batch",
                fresh_events_in_batch
            );
        } else {
            println!("⏳ No fresh events in this batch (all events are historical)");
        }

        // Stop if we found the requested number of fresh events from our target package
        if target_events_processed >= num_events {
            println!(
                "\n🎉 Found {} fresh events from target package, stopping search",
                target_events_processed
            );
            break;
        }

        // Update cursor for next query (to get newer events)
        cursor = response.next_cursor;

        // If no fresh events found, reset cursor and wait before next poll
        if fresh_events_in_batch == 0 {
            cursor = None; // Reset to get most recent events
            sleep(polling_interval).await;
        } else {
            // If we found fresh events, continue quickly to catch up
            sleep(Duration::from_millis(500)).await;
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
    println!("   • Fresh events found: {}", fresh_events_found);
    println!(
        "   • Fresh events from target package: {}",
        target_events_processed
    );
    println!("   • Average delay: {:.2}ms", average_delay);
    println!(
        "   • Start time: {} ({}ms)",
        start_time.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
        start_timestamp_ms
    );
    println!("   • Server-side filtering: ✅ Enabled");

    Ok((target_events_processed, average_delay, total_response_size))
}
#[allow(dead_code)]
async fn query_recent_events_with_size(
    client: &reqwest::Client,
    rpc_url: &str,
    cursor: Option<&Value>,
) -> Result<(QueryEventsResponse, u64), Box<dyn std::error::Error>> {
    // Re-enabled: Use server-side module filtering for dubhe_dex_system module
    let filter = json!({
        "MoveModule": {
            "package": TARGET_PACKAGE_ID,
            "module": "dubhe_dex_system"
        }
    });

    let params = if cursor.is_some() {
        json!([filter, cursor, 100, true]) // descending=true for most recent first
    } else {
        json!([filter, null, 100, true])
    };

    let request_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "suix_queryEvents",
        "params": params
    });

    let response = client
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let response_text = response.text().await?;
    let response_size = response_text.len() as u64;
    let response_json: Value = serde_json::from_str(&response_text)?;

    if let Some(error) = response_json.get("error") {
        return Err(format!("RPC Error: {}", error).into());
    }

    let result = response_json["result"].clone();
    let parsed_response: QueryEventsResponse = serde_json::from_value(result)?;
    Ok((parsed_response, response_size))
}
#[allow(dead_code)]
fn display_event(event: &SuiEvent, event_number: u32) -> Option<i64> {
    println!("\n🎉 Event #{} found from target package!", event_number);
    println!("┌─────────────────────────────────────────────────────────────────");
    println!("│ Transaction: {}", event.id.tx_digest);
    println!("│ Package ID:  {}", event.package_id);
    println!("│ Module:      {}", event.transaction_module);
    println!("│ Event Type:  {}", event.event_type);
    println!("│ Sender:      {}", event.sender);
    println!("│ Event Seq:   {}", event.id.event_seq);

    // Calculate and display timestamp with delay
    let calculated_delay = if let Some(timestamp_str) = &event.timestamp_ms {
        if let Ok(timestamp_ms) = timestamp_str.parse::<i64>() {
            // Convert milliseconds to seconds for DateTime::from_timestamp
            let timestamp_secs = timestamp_ms / 1000;
            let timestamp_nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;

            if let Some(datetime) = DateTime::from_timestamp(timestamp_secs, timestamp_nanos) {
                // Calculate delay from event creation to now
                let now = Utc::now();
                let delay = now - datetime;
                let delay_ms = delay.num_milliseconds();

                println!(
                    "│ Timestamp:   {} (delay: {}ms)",
                    datetime.format("%Y-%m-%d %H:%M:%S%.3f UTC"),
                    delay_ms
                );
                Some(delay_ms)
            } else {
                println!("│ Timestamp:   {} ms (invalid)", timestamp_str);
                None
            }
        } else {
            println!("│ Timestamp:   {} ms (invalid format)", timestamp_str);
            None
        }
    } else {
        None
    };

    // Display JSON content if available
    // if let Some(json_content) = &event.parsed_json {
    //     println!("│ JSON Data:");
    //     let json_str = serde_json::to_string_pretty(json_content)
    //         .unwrap_or_else(|_| "Failed to serialize JSON".to_string());
    //     for line in json_str.lines() {
    //         println!("│   {}", line);
    //     }
    // }

    // Display link to Suiscan
    println!(
        "│ 🌐 View on Suiscan: https://suiscan.xyz/testnet/tx/{}",
        event.id.tx_digest
    );
    println!("└─────────────────────────────────────────────────────────────────");

    calculated_delay
}

mod sui;
mod tsa;

use chrono::{DateTime, NaiveDateTime, Utc};
use digicert_tsa::get_timestamp;
use dotenvy::dotenv;
use serde_json;
use sha2::{Digest, Sha256};
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default configuration: info level for the application, warn for dependencies
        tracing_subscriber::EnvFilter::new("debug,hyper=warn,h2=warn,tower=warn,reqwest=warn")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true) // Include the module path in logs
        .with_thread_ids(true) // Include thread IDs
        .with_level(true) // Include log level
        .with_file(true) // Include file name
        .with_line_number(true) // Include line number
        .init();

    info!("Starting TSA checker");

    let endpoints = ["https://freetsa.org/tsr", "http://timestamp.digicert.com"];
    let full_node_url = "https://fullnode.testnet.sui.io";

    for endpoint in &endpoints {
        info!("\n=== Testing TSA: {} ===", endpoint);

        // Get a fresh checkpoint for each endpoint
        debug!("Fetching latest checkpoint...");
        let checkpoint = sui::get_last_checkpoint(full_node_url)
            .await
            .map_err(|e| anyhow::anyhow!("Error getting latest checkpoint: {}", e))?;

        debug!("Latest checkpoint sequence: {}", checkpoint.sequence_number);
        debug!("Checkpoint timestamp: {} ms", checkpoint.timestamp_ms);
        debug!("Checkpoint digest: {}", checkpoint.digest);

        // Combine checkpoint data with our data and hash it
        let data = b"Hello, world!";
        let checkpoint_json = serde_json::to_string(&checkpoint)?;
        let checkpoint_bytes = checkpoint_json.as_bytes();
        let combined_data = [checkpoint_bytes, data].concat();

        debug!(
            "Data to timestamp: full checkpoint JSON + {:?}",
            std::str::from_utf8(data).unwrap_or("(binary data)")
        );
        debug!("Full checkpoint JSON: {}", checkpoint_json);
        debug!("Combined data length: {} bytes", combined_data.len());

        // Hash the combined data
        let hash = Sha256::digest(&combined_data);
        debug!("SHA256 hash: {}", hex::encode(hash));

        // Call the async TSA function directly (no spawn_blocking needed)
        let response = get_timestamp(&combined_data, endpoint).await?;

        debug!("✅ Timestamp successful!");
        debug!("TSA genTime  : {:?}", response.gen_time);
        debug!("Time string  : {}", response.time_string);

        // Parse checkpoint timestamp (milliseconds) - moved inside since checkpoint is in loop scope
        let checkpoint_time_ms: u64 = checkpoint
            .timestamp_ms
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse checkpoint timestamp: {}", e))?;

        // Parse TSA time (assume it's in format YYYYMMDDHHMMSSZ)
        // Convert to milliseconds since epoch for comparison
        let tsa_time_str = &response.time_string.replace("Z", "");

        if tsa_time_str.len() >= 14 {
            // Parse using chrono for accurate datetime conversion
            if let Ok(naive_dt) = NaiveDateTime::parse_from_str(tsa_time_str, "%Y%m%d%H%M%S") {
                let tsa_datetime: DateTime<Utc> =
                    DateTime::from_naive_utc_and_offset(naive_dt, Utc);
                let tsa_time_ms = tsa_datetime.timestamp_millis() as u64;

                debug!("Checkpoint time: {} ms", checkpoint_time_ms);
                debug!("TSA time: {} ms", tsa_time_ms);

                let time_diff = if tsa_time_ms > checkpoint_time_ms {
                    tsa_time_ms - checkpoint_time_ms
                } else {
                    checkpoint_time_ms - tsa_time_ms
                };

                info!(
                    "Time difference: {} ms ({:.2} seconds)",
                    time_diff,
                    time_diff as f64 / 1000.0
                );

                // Also show human-readable times for verification
                let checkpoint_dt =
                    DateTime::<Utc>::from_timestamp_millis(checkpoint_time_ms as i64)
                        .unwrap_or_default();
                debug!(
                    "Checkpoint time (UTC): {}",
                    checkpoint_dt.format("%Y-%m-%d %H:%M:%S%.3f")
                );
                debug!(
                    "TSA time (UTC): {}",
                    tsa_datetime.format("%Y-%m-%d %H:%M:%S")
                );
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to parse TSA time format: {}",
                    tsa_time_str
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Could not parse TSA time format: {}",
                tsa_time_str
            ));
        }

        // Save the response
        // let filename = format!("timestamp_checkpoint_{}.tsr", checkpoint.sequence_number);
        // std::fs::write(&filename, &response.full_response)?;
        // debug!("Saved response to: {}", filename);

        // Fetch checkpoint again to see if it changed
        debug!("\nWaiting 5 seconds before fetching same checkpoint again...");
        sleep(Duration::from_secs(5)).await;

        debug!("Fetching same checkpoint again for comparison...");
        let new_checkpoint =
            sui::get_checkpoint(checkpoint.sequence_number.parse()?, full_node_url)
                .await
                .map_err(|e| anyhow::anyhow!("Error fetching same checkpoint again: {}", e))?;

        // Hash the entire checkpoint objects for comparison
        let original_checkpoint_json = serde_json::to_string(&checkpoint)?;
        let new_checkpoint_json = serde_json::to_string(&new_checkpoint)?;
        debug!("Original checkpoint JSON: {}", original_checkpoint_json);
        debug!("New checkpoint JSON: {}", new_checkpoint_json);

        let original_hash = Sha256::digest(original_checkpoint_json.as_bytes());
        let new_hash = Sha256::digest(new_checkpoint_json.as_bytes());

        if original_hash != new_hash {
            error!("⚠️  Checkpoint data changed! Same sequence but different content!");
            debug!("Original hash: {}", hex::encode(original_hash));
            debug!("New hash: {}", hex::encode(new_hash));

            // Compare all fields individually and report what changed
            if checkpoint.checkpoint_commitments != new_checkpoint.checkpoint_commitments {
                error!("❌ checkpoint_commitments changed:");
                error!("   Original: {:?}", checkpoint.checkpoint_commitments);
                error!("   New: {:?}", new_checkpoint.checkpoint_commitments);
            }

            if checkpoint.digest != new_checkpoint.digest {
                error!("❌ digest changed:");
                error!("   Original: {}", checkpoint.digest);
                error!("   New: {}", new_checkpoint.digest);
            }

            if checkpoint.epoch != new_checkpoint.epoch {
                error!("❌ epoch changed:");
                error!("   Original: {}", checkpoint.epoch);
                error!("   New: {}", new_checkpoint.epoch);
            }

            // Compare epoch_rolling_gas_cost_summary fields
            if checkpoint.epoch_rolling_gas_cost_summary.computation_cost
                != new_checkpoint
                    .epoch_rolling_gas_cost_summary
                    .computation_cost
            {
                error!("❌ epoch_rolling_gas_cost_summary.computation_cost changed:");
                error!(
                    "   Original: {}",
                    checkpoint.epoch_rolling_gas_cost_summary.computation_cost
                );
                error!(
                    "   New: {}",
                    new_checkpoint
                        .epoch_rolling_gas_cost_summary
                        .computation_cost
                );
            }

            if checkpoint
                .epoch_rolling_gas_cost_summary
                .non_refundable_storage_fee
                != new_checkpoint
                    .epoch_rolling_gas_cost_summary
                    .non_refundable_storage_fee
            {
                error!("❌ epoch_rolling_gas_cost_summary.non_refundable_storage_fee changed:");
                error!(
                    "   Original: {}",
                    checkpoint
                        .epoch_rolling_gas_cost_summary
                        .non_refundable_storage_fee
                );
                error!(
                    "   New: {}",
                    new_checkpoint
                        .epoch_rolling_gas_cost_summary
                        .non_refundable_storage_fee
                );
            }

            if checkpoint.epoch_rolling_gas_cost_summary.storage_cost
                != new_checkpoint.epoch_rolling_gas_cost_summary.storage_cost
            {
                error!("❌ epoch_rolling_gas_cost_summary.storage_cost changed:");
                error!(
                    "   Original: {}",
                    checkpoint.epoch_rolling_gas_cost_summary.storage_cost
                );
                error!(
                    "   New: {}",
                    new_checkpoint.epoch_rolling_gas_cost_summary.storage_cost
                );
            }

            if checkpoint.epoch_rolling_gas_cost_summary.storage_rebate
                != new_checkpoint.epoch_rolling_gas_cost_summary.storage_rebate
            {
                error!("❌ epoch_rolling_gas_cost_summary.storage_rebate changed:");
                error!(
                    "   Original: {}",
                    checkpoint.epoch_rolling_gas_cost_summary.storage_rebate
                );
                error!(
                    "   New: {}",
                    new_checkpoint.epoch_rolling_gas_cost_summary.storage_rebate
                );
            }

            if checkpoint.network_total_transactions != new_checkpoint.network_total_transactions {
                error!("❌ network_total_transactions changed:");
                error!("   Original: {}", checkpoint.network_total_transactions);
                error!("   New: {}", new_checkpoint.network_total_transactions);
            }

            if checkpoint.previous_digest != new_checkpoint.previous_digest {
                error!("❌ previous_digest changed:");
                error!("   Original: {}", checkpoint.previous_digest);
                error!("   New: {}", new_checkpoint.previous_digest);
            }

            if checkpoint.sequence_number != new_checkpoint.sequence_number {
                error!("❌ sequence_number changed:");
                error!("   Original: {}", checkpoint.sequence_number);
                error!("   New: {}", new_checkpoint.sequence_number);
            }

            if checkpoint.timestamp_ms != new_checkpoint.timestamp_ms {
                error!("❌ timestamp_ms changed:");
                error!("   Original: {}", checkpoint.timestamp_ms);
                error!("   New: {}", new_checkpoint.timestamp_ms);
            }

            if checkpoint.transactions != new_checkpoint.transactions {
                error!("❌ transactions changed:");
                error!("   Original count: {}", checkpoint.transactions.len());
                error!("   New count: {}", new_checkpoint.transactions.len());
                if checkpoint.transactions.len() != new_checkpoint.transactions.len() {
                    error!("   Transaction count differs!");
                } else {
                    // Compare individual transactions
                    for (i, (orig_tx, new_tx)) in checkpoint
                        .transactions
                        .iter()
                        .zip(new_checkpoint.transactions.iter())
                        .enumerate()
                    {
                        if orig_tx != new_tx {
                            error!("   Transaction {} changed: {} -> {}", i, orig_tx, new_tx);
                        }
                    }
                }
            }

            if checkpoint.validator_signature != new_checkpoint.validator_signature {
                error!("❌ validator_signature changed:");
                error!("   Original: {}", checkpoint.validator_signature);
                error!("   New: {}", new_checkpoint.validator_signature);
            }
        } else {
            info!(
                "✅ Checkpoint data completely identical (sequence: {}, hash: {})",
                checkpoint.sequence_number,
                hex::encode(original_hash)
            );

            // Also fetch the latest checkpoint to see progression
            let latest_checkpoint = sui::get_last_checkpoint(full_node_url)
                .await
                .map_err(|e| anyhow::anyhow!("Error fetching latest checkpoint: {}", e))?;

            if latest_checkpoint.sequence_number != checkpoint.sequence_number {
                debug!(
                    "📈 Latest checkpoint progressed from {} to {}",
                    checkpoint.sequence_number, latest_checkpoint.sequence_number
                );
                let latest_time_ms: u64 = latest_checkpoint.timestamp_ms.parse().unwrap_or(0);
                let checkpoint_diff = if latest_time_ms > checkpoint_time_ms {
                    latest_time_ms - checkpoint_time_ms
                } else {
                    checkpoint_time_ms - latest_time_ms
                };
                debug!(
                    "Time progression: {} ms ({:.2} seconds)",
                    checkpoint_diff,
                    checkpoint_diff as f64 / 1000.0
                );
            } else {
                debug!("📊 No new checkpoints since our fetch");
            }
        }
    }

    Ok(())
}

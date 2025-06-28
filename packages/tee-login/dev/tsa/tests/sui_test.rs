use anyhow::Result;
use dotenvy::dotenv;
use serde_json;
use sha2::{Digest, Sha256};
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info};

// Import modules from the main crate
use digicert_tsa::sui;

#[tokio::test]
async fn test_checkpoint_stability() -> Result<()> {
    dotenv().ok();
    // Initialize tracing for the test
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("debug,hyper=warn,h2=warn,tower=warn,reqwest=warn")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Sui checkpoint stability test");

    let full_node_url = "https://rpc-mainnet.suiscan.xyz"; //"https://fullnode.mainnet.sui.io"; //"https://rpc-mainnet.suiscan.xyz"; 
    let mut differences_found = false;
    let mut validator_signatures_changed = 0;
    let mut max_validator_signature_change_time: u64 = 0;
    let mut max_validator_signature_change_count = 0;
    // let checkpoint = sui::get_checkpoint(160983410, full_node_url)
    //     .await
    //     .map_err(|e| anyhow::anyhow!("Error getting checkpoint: {}", e))?;
    // info!("Checkpoint: {:?}", checkpoint);
    // let committee_info = sui::get_committee_info(checkpoint.epoch.parse()?, full_node_url)
    //     .await
    //     .map_err(|e| anyhow::anyhow!("Error getting committee info: {}", e))?;
    // info!("Committee info: {:?}", committee_info);

    //let signers_map = sui::extract_signers_map(&checkpoint.validator_signature);
    //info!("Signers map: {:?}", signers_map);
    sleep(Duration::from_secs(15)).await;

    for iteration in 1..=1000 {
        info!("🔄 Iteration {}/1000", iteration);

        // Get the latest checkpoint
        debug!("Fetching latest checkpoint...");
        let checkpoint = sui::get_last_checkpoint(full_node_url)
            .await
            .map_err(|e| anyhow::anyhow!("Error getting latest checkpoint: {}", e))?;

        info!("Latest checkpoint sequence: {}", checkpoint.sequence_number);
        debug!("Checkpoint timestamp: {} ms", checkpoint.timestamp_ms);
        debug!("Checkpoint digest: {}", checkpoint.digest);

        // Wait 15 seconds
        debug!("⏱️  Waiting 15 seconds before fetching same checkpoint again...");
        sleep(Duration::from_secs(15)).await;

        // Fetch the same checkpoint again by sequence number
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
            error!(
                "⚠️  Checkpoint data changed! Same sequence but different content! (Iteration {})",
                iteration
            );
            debug!("Original hash: {}", hex::encode(original_hash));
            debug!("New hash: {}", hex::encode(new_hash));

            // Compare all fields individually and report what changed
            if checkpoint.checkpoint_commitments != new_checkpoint.checkpoint_commitments {
                error!("❌ checkpoint_commitments changed:");
                error!("   Original: {:?}", checkpoint.checkpoint_commitments);
                error!("   New: {:?}", new_checkpoint.checkpoint_commitments);
                differences_found = true;
            }

            if checkpoint.digest != new_checkpoint.digest {
                error!("❌ digest changed:");
                error!("   Original: {}", checkpoint.digest);
                error!("   New: {}", new_checkpoint.digest);
                differences_found = true;
            }

            if checkpoint.epoch != new_checkpoint.epoch {
                error!("❌ epoch changed:");
                error!("   Original: {}", checkpoint.epoch);
                error!("   New: {}", new_checkpoint.epoch);
                differences_found = true;
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
                differences_found = true;
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
                differences_found = true;
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
                differences_found = true;
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
                differences_found = true;
            }

            if checkpoint.network_total_transactions != new_checkpoint.network_total_transactions {
                error!("❌ network_total_transactions changed:");
                error!("   Original: {}", checkpoint.network_total_transactions);
                error!("   New: {}", new_checkpoint.network_total_transactions);
                differences_found = true;
            }

            if checkpoint.previous_digest != new_checkpoint.previous_digest {
                error!("❌ previous_digest changed:");
                error!("   Original: {}", checkpoint.previous_digest);
                error!("   New: {}", new_checkpoint.previous_digest);
                differences_found = true;
            }

            if checkpoint.sequence_number != new_checkpoint.sequence_number {
                error!("❌ sequence_number changed:");
                error!("   Original: {}", checkpoint.sequence_number);
                error!("   New: {}", new_checkpoint.sequence_number);
                differences_found = true;
            }

            if checkpoint.timestamp_ms != new_checkpoint.timestamp_ms {
                error!("❌ timestamp_ms changed:");
                error!("   Original: {}", checkpoint.timestamp_ms);
                error!("   New: {}", new_checkpoint.timestamp_ms);
                differences_found = true;
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
                differences_found = true;
            }

            if checkpoint.validator_signature != new_checkpoint.validator_signature {
                error!("❌ validator_signature changed:");
                error!("   Original: {}", checkpoint.validator_signature);
                error!("   New: {}", new_checkpoint.validator_signature);
                validator_signatures_changed += 1;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let checkpoint_time_ms: u64 = checkpoint.timestamp_ms.parse().unwrap_or(0);
                let mut time_diff = now_ms.saturating_sub(checkpoint_time_ms);
                let mut previous_signature = new_checkpoint.validator_signature.clone();
                sleep(Duration::from_secs(15)).await;

                let mut one_more_checkpoint =
                    sui::get_checkpoint(checkpoint.sequence_number.parse()?, full_node_url)
                        .await
                        .map_err(|e| {
                            anyhow::anyhow!("Error fetching same checkpoint again: {}", e)
                        })?;
                let mut unchanged_count = 0;
                let mut changed_count = 1;
                while (one_more_checkpoint.validator_signature != previous_signature)
                    || (unchanged_count < 40)
                {
                    if one_more_checkpoint.validator_signature != previous_signature {
                        error!("❌ validator_signature changed again:");
                        error!("   Previous: {}", previous_signature);
                        error!("   New: {}", one_more_checkpoint.validator_signature);
                        previous_signature = one_more_checkpoint.validator_signature.clone();
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        time_diff = now_ms.saturating_sub(checkpoint_time_ms);
                        error!("❌ time_diff: {}", time_diff);
                        unchanged_count = 0;
                        changed_count += 1;
                    } else {
                        unchanged_count += 1;
                    }
                    sleep(Duration::from_secs(15)).await;
                    one_more_checkpoint =
                        sui::get_checkpoint(checkpoint.sequence_number.parse()?, full_node_url)
                            .await
                            .map_err(|e| {
                                anyhow::anyhow!("Error fetching same checkpoint again: {}", e)
                            })?;
                }
                info!("❌ time_diff final: {}", time_diff);
                if time_diff > max_validator_signature_change_time {
                    max_validator_signature_change_time = time_diff;
                }
                if changed_count > max_validator_signature_change_count {
                    max_validator_signature_change_count = changed_count;
                }
                info!(
                    "❌ max_validator_signature_change_time: {}",
                    max_validator_signature_change_time
                );
                info!(
                    "❌ max_validator_signature_change_count: {}",
                    max_validator_signature_change_count
                );
            }
        } else {
            info!(
                "✅ Iteration {}: Checkpoint data completely identical (sequence: {}, hash: {})",
                iteration,
                checkpoint.sequence_number,
                hex::encode(original_hash)
            );
        }
        if differences_found {
            info!(
                "❌ validator_signatures_change_count: {}",
                max_validator_signature_change_count
            );
            info!(
                "❌ validator_signatures_changed: {}",
                validator_signatures_changed
            );
            info!(
                "❌ max_validator_signature_change_time: {}",
                max_validator_signature_change_time
            );
            panic!(
                "❌ TEST FAILED: Checkpoint differences were detected during the stability test!"
            );
        }
    }

    info!(
        "❌ validator_signatures_change_count: {}",
        max_validator_signature_change_count
    );
    info!(
        "❌ validator_signatures_changed: {}",
        validator_signatures_changed
    );
    info!(
        "❌ max_validator_signature_change_time: {}",
        max_validator_signature_change_time
    );
    // Test fails if any differences were found
    if differences_found {
        panic!("❌ TEST FAILED: Checkpoint differences were detected during the stability test!");
    } else {
        info!("✅ TEST PASSED: All 10 iterations showed identical checkpoint data");
    }

    Ok(())
}

//! Sha256n command handler - Iteratively hash n times with timing measurements

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::time::Instant;
use tracing::info;

use crate::{contract, url};

/// Handle the sha256n command with timing measurements
///
/// # Arguments
/// * `numbers` - Array of integers to hash (will be converted to hex)
/// * `count` - Number of iterations to perform
///
/// # Returns
/// Result indicating success or failure
pub async fn handle_sha256n(numbers: Vec<i64>, count: i64) -> Result<()> {
    info!("Computing SHA256 hash {} times for {} numbers", count, numbers.len());

    // Convert integers to hex with even-length padding
    let hex_strings: Vec<String> = numbers
        .iter()
        .map(|n| {
            let hex = format!("{:x}", n);
            if hex.len() % 2 == 0 {
                hex
            } else {
                format!("0{}", hex)
            }
        })
        .collect();

    info!("Hex strings: {:?}", hex_strings);

    // Compute expected hash in Rust for verification
    let concatenated = hex_strings.join("");

    // Compute n iterations locally
    // Daml's Crypto.sha256 decodes hex strings to bytes first
    let mut rust_hash = {
        let bytes = hex::decode(&concatenated)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };

    for i in 1..count {
        // Concatenate hash with original input (both as hex strings)
        let combined = format!("{}{}", rust_hash, concatenated);
        // Decode the concatenated hex string to bytes
        let bytes = hex::decode(&combined)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        rust_hash = hex::encode(hasher.finalize());

        if i % 10 == 0 {
            info!("Rust iteration {}/{}", i + 1, count);
        }
    }

    info!("Expected final hash (Rust): {}", rust_hash);

    // Load environment variables
    let api_url = std::env::var("APP_USER_API_URL")
        .context("APP_USER_API_URL not set in environment")?;
    let jwt = std::env::var("APP_USER_JWT")
        .context("APP_USER_JWT not set in environment")?;
    let party_app_user = std::env::var("PARTY_APP_USER")
        .context("PARTY_APP_USER not set in environment")?;
    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .context("PARTY_APP_PROVIDER not set in environment")?;
    let hash_package_id = std::env::var("HASH_PACKAGE_ID")
        .context("HASH_PACKAGE_ID not set in environment")?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .context("SYNCHRONIZER_ID not set in environment")?;

    let template_id = format!("{}:Hash:Hash", hash_package_id);

    // Create HTTP client with localhost resolution
    let client = url::create_client_with_localhost_resolution()?;

    info!("Creating Hash contract...");

    // Create Hash contract
    let (contract_id, _hash_id, _create_update_id, _create_update) = contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_app_provider,
        &template_id,
        &synchronizer_id,
    )
    .await?;

    // Prepare choice argument
    let choice_argument = serde_json::json!({
        "hexStrings": hex_strings,
        "count": count.to_string()
    });

    info!("Exercising Sha256n choice with count={}...", count);
    info!("Starting timer for Canton execution...");

    // Start timing the Canton choice execution
    let start = Instant::now();

    // Exercise Sha256n choice
    let update_id = contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &contract_id,
        "Sha256n",
        choice_argument,
    )
    .await?;

    // Stop timer
    let duration = start.elapsed();
    let total_ms = duration.as_millis();
    let per_hash_ms = total_ms as f64 / count as f64;

    info!("Canton execution completed");

    // Get and display the update
    let update = contract::get_update(&client, &api_url, &jwt, &party_app_user, &update_id).await?;

    // Extract the hash from contract state
    if let Some(events) = update
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(daml_hash) = created
                    .pointer("/createArgument/sha256_result")
                    .and_then(|v| v.as_str())
                {
                    println!("\n=== Sha256n Results ===");
                    println!("Count:                 {}", count);
                    println!("Input numbers:         {:?}", numbers);
                    println!("Input hex strings:     {:?}", hex_strings);
                    println!();
                    println!("Daml hash (final):     {}", daml_hash);
                    println!("Rust hash (expected):  {}", rust_hash);
                    println!();

                    if daml_hash == rust_hash {
                        println!("✅ Hashes match!");
                    } else {
                        println!("❌ Hash mismatch!");
                    }

                    println!();
                    println!("=== Performance Metrics ===");
                    println!("Total Canton time:     {} ms", total_ms);
                    println!("Time per hash:         {:.2} ms", per_hash_ms);
                    println!("Hashes per second:     {:.2}", 1000.0 / per_hash_ms);

                    return Ok(());
                }
            }
        }
    }

    Err(anyhow::anyhow!("Could not find sha256_result in update"))
}

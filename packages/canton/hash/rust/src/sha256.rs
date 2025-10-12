use anyhow::Result;
use serde_json::json;
use sha2::{Sha256, Digest};
use tracing::info;

pub async fn handle_sha256(numbers: Vec<i64>) -> Result<()> {
    info!("Computing SHA256 hash for numbers: {:?}", numbers);

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let template_id = format!("{}:Hash:Hash", package_id);

    // Convert integers to hex strings with even length (required for valid hex)
    let hex_strings: Vec<String> = numbers
        .iter()
        .map(|n| {
            let hex = format!("{:x}", n);
            // Ensure even length by padding with leading zero if needed
            if hex.len() % 2 == 0 {
                hex
            } else {
                format!("0{}", hex)
            }
        })
        .collect();

    info!("Hex strings: {:?}", hex_strings);

    // Calculate sha256 hash in Rust for comparison
    // Daml's sha256 decodes the hex string to bytes first, so we do the same
    let concatenated = hex_strings.concat();
    let bytes = hex::decode(&concatenated)
        .map_err(|e| anyhow::anyhow!("Failed to decode hex string: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let rust_hash = hasher.finalize();
    let rust_hash_hex = hex::encode(rust_hash);

    info!("Rust calculated hash: {}", rust_hash_hex);

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Create Hash contract
    let hash_contract_id = crate::contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
    ).await?;

    // Exercise Sha256 choice
    let choice_argument = json!({ "hexStrings": hex_strings });
    let sha256_update_id = crate::contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &hash_contract_id,
        "Sha256",
        choice_argument,
    ).await?;

    // Get the update result
    let result_json = crate::contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &sha256_update_id,
    ).await?;

    // Print the full update
    println!("\n=== Sha256 Choice Update ===");
    println!("{}", serde_json::to_string_pretty(&result_json)?);

    // Extract and display the sha256 result
    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Hash:Hash") {
                        if let Some(sha256_result) = created.pointer("/createArgument/sha256_result") {
                            let daml_hash = sha256_result.as_str().unwrap_or("null");

                            println!("\n=== Result ===");
                            println!("Input numbers: {:?}", numbers);
                            println!("Hex strings: {:?}", hex_strings);
                            println!("Concatenated: {}", concatenated);
                            println!("Rust calculated hash:  {}", rust_hash_hex);
                            println!("Daml calculated hash:  {}", daml_hash);

                            if rust_hash_hex == daml_hash {
                                println!("✅ Hashes match!");
                            } else {
                                println!("❌ Hashes DO NOT match!");
                            }

                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

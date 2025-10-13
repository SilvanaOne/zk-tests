use anyhow::Result;
use serde_json::json;
use tiny_keccak::{Hasher, Keccak};
use tracing::info;

pub async fn handle_keccak(numbers: Vec<i64>) -> Result<()> {
    info!("Computing Keccak256 hash for numbers: {:?}", numbers);

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

    // Calculate keccak256 hash in Rust for comparison
    // Daml's keccak256 decodes the hex string to bytes first, so we do the same
    let concatenated = hex_strings.concat();
    let bytes = hex::decode(&concatenated)
        .map_err(|e| anyhow::anyhow!("Failed to decode hex string: {}", e))?;

    let mut hasher = Keccak::v256();
    hasher.update(&bytes);
    let mut rust_hash = [0u8; 32];
    hasher.finalize(&mut rust_hash);
    let rust_hash_hex = hex::encode(rust_hash);

    info!("Rust calculated hash: {}", rust_hash_hex);

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Create Hash contract
    let (hash_contract_id, _create_update) = crate::contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
    ).await?;

    // Exercise Keccak choice
    let choice_argument = json!({ "hexStrings": hex_strings });
    let keccak_update_id = crate::contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &hash_contract_id,
        "Keccak",
        choice_argument,
    ).await?;

    // Get the update result
    let result_json = crate::contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &keccak_update_id,
    ).await?;

    // Print the full update
    println!("\n=== Keccak Choice Update ===");
    println!("{}", serde_json::to_string_pretty(&result_json)?);

    // Extract and display the keccak result
    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Hash:Hash") {
                        if let Some(keccak_result) = created.pointer("/createArgument/keccak_result") {
                            let daml_hash = keccak_result.as_str().unwrap_or("null");

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

use anyhow::Result;
use serde_json::json;
use tracing::info;

pub async fn handle_add(numbers: Vec<i64>) -> Result<()> {
    info!("Adding numbers: {:?}", numbers);

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PROVIDER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let template_id = format!("{}:Hash:Hash", package_id);

    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set in environment"))?;

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Create Hash contract
    let (hash_contract_id, _hash_id, _create_update_id, _create_update) = crate::contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_app_provider,
        &template_id,
        &synchronizer_id,
    ).await?;

    // Exercise Add choice
    let choice_argument = json!({ "numbers": numbers });
    let add_update_id = crate::contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &hash_contract_id,
        "Add",
        choice_argument,
    ).await?;

    // Get the update result
    let result_json = crate::contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &add_update_id,
    ).await?;

    // Print the full update
    println!("\n=== Add Choice Update ===");
    println!("{}", serde_json::to_string_pretty(&result_json)?);

    // Extract and display the sum result
    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Hash:Hash") {
                        if let Some(add_result) = created.pointer("/createArgument/add_result") {
                            println!("\n=== Result ===");
                            println!("Numbers: {:?}", numbers);
                            println!("Sum (add_result): {}", add_result);
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

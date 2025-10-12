use anyhow::Result;
use serde_json::json;
use tracing::{info, debug};

pub async fn handle_add(numbers: Vec<i64>) -> Result<()> {
    info!("Adding numbers: {:?}", numbers);

    // Get PARTY_APP_USER from environment
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    info!("Creating Hash contract for party: {}", party_app_user);

    // Create HTTP client with localhost resolution
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Step 1: Create Hash contract
    let create_cmdid = format!("create-hash-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:Hash:Hash", package_id);

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": &template_id,
                "createArguments": {
                    "owner": party_app_user,
                    "add_result": 0
                }
            }
        }],
        "commandId": create_cmdid,
        "actAs": [&party_app_user],
        "readAs": []
    });

    info!("Submitting Hash contract creation");
    debug!("Create payload: {}", serde_json::to_string_pretty(&create_payload)?);

    let create_response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(&jwt)
        .json(&create_payload)
        .send()
        .await?;

    let create_status = create_response.status();
    let create_text = create_response.text().await?;

    if !create_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to create Hash contract: HTTP {} - {}",
            create_status,
            create_text
        ));
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    let create_update_id = create_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in create response"))?;

    info!("Hash contract created, updateId: {}", create_update_id);

    // Step 2: Get the contract ID from the update
    let update_payload = json!({
        "actAs": [&party_app_user],
        "updateId": create_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app_user: {}
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    debug!("Fetching update to get contract ID");
    let update_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(&jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract Hash contract ID
    let mut hash_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Hash:Hash") {
                        hash_cid = created.pointer("/contractId").and_then(|v| v.as_str());
                        break;
                    }
                }
            }
        }
    }

    let hash_contract_id = hash_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find Hash contract in create update"))?;

    info!("Hash contract ID: {}", hash_contract_id);

    // Step 3: Exercise Add choice with the numbers
    let add_cmdid = format!("add-hash-{}", chrono::Utc::now().timestamp());

    let add_payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": &template_id,
                "contractId": hash_contract_id,
                "choice": "Add",
                "choiceArgument": {
                    "numbers": numbers
                }
            }
        }],
        "commandId": add_cmdid,
        "actAs": [&party_app_user],
        "readAs": []
    });

    info!("Exercising Add choice with numbers: {:?}", numbers);
    debug!("Add payload: {}", serde_json::to_string_pretty(&add_payload)?);

    let add_response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(&jwt)
        .json(&add_payload)
        .send()
        .await?;

    let add_status = add_response.status();
    let add_text = add_response.text().await?;

    if !add_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to exercise Add choice: HTTP {} - {}",
            add_status,
            add_text
        ));
    }

    let add_json: serde_json::Value = serde_json::from_str(&add_text)?;
    let add_update_id = add_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in add response"))?;

    info!("Add choice executed, updateId: {}", add_update_id);

    // Step 4: Get the update to see the result
    let result_payload = json!({
        "actAs": [&party_app_user],
        "updateId": add_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app_user: {}
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    debug!("Fetching add update to see result");
    let result_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(&jwt)
        .json(&result_payload)
        .send()
        .await?;

    let result_text = result_response.text().await?;
    let result_json: serde_json::Value = serde_json::from_str(&result_text)?;

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

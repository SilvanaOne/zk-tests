use anyhow::Result;
use serde_json::json;
use tracing::{info, debug};

pub async fn create_hash_contract(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    template_id: &str,
) -> Result<String> {
    let create_cmdid = format!("create-hash-{}", chrono::Utc::now().timestamp());

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "owner": party_app_user,
                    "add_result": 0,
                    "keccak_result": null
                }
            }
        }],
        "commandId": create_cmdid,
        "actAs": [party_app_user],
        "readAs": []
    });

    info!("Submitting Hash contract creation");
    debug!("Create payload: {}", serde_json::to_string_pretty(&create_payload)?);

    let create_response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(jwt)
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

    // Get the contract ID from the update
    let update_payload = json!({
        "actAs": [party_app_user],
        "updateId": create_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        party_app_user: {}
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
        .bearer_auth(jwt)
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
                        hash_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let hash_contract_id = hash_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find Hash contract in create update"))?;

    info!("Hash contract ID: {}", hash_contract_id);

    Ok(hash_contract_id)
}

pub async fn exercise_choice(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    template_id: &str,
    contract_id: &str,
    choice_name: &str,
    choice_argument: serde_json::Value,
) -> Result<String> {
    let cmdid = format!("{}-hash-{}", choice_name.to_lowercase(), chrono::Utc::now().timestamp());

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": contract_id,
                "choice": choice_name,
                "choiceArgument": choice_argument
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": []
    });

    info!("Exercising {} choice", choice_name);
    debug!("Choice payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to exercise {} choice: HTTP {} - {}",
            choice_name,
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("{} choice executed, updateId: {}", choice_name, update_id);

    Ok(update_id.to_string())
}

pub async fn get_update(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    update_id: &str,
) -> Result<serde_json::Value> {
    let payload = json!({
        "actAs": [party_app_user],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        party_app_user: {}
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    debug!("Fetching update");
    let response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;

    Ok(json)
}

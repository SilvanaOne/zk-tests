use anyhow::Result;
use serde_json::json;
use tracing::{info, debug};

pub async fn create_hash_contract(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    party_app_provider: &str,
    template_id: &str,
    synchronizer_id: &str,
) -> Result<(String, String, String, serde_json::Value)> {
    let create_cmdid = format!("create-hash-request-{}", chrono::Utc::now().timestamp());
    let hash_id = uuid::Uuid::new_v4().to_string();

    info!("Creating HashRequest with id: {}", hash_id);

    // Extract package ID for HashRequest template
    let package_id = template_id.split(':').next()
        .ok_or_else(|| anyhow::anyhow!("Invalid template_id format"))?;
    let hash_request_template_id = format!("{}:HashRequest:HashRequest", package_id);

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": hash_request_template_id,
                "createArguments": {
                    "owner": party_app_user,
                    "provider": party_app_provider,
                    "id": hash_id,
                    "add_result": 0,
                    "keccak_result": null,
                    "sha256_result": null,
                    "root": "57ab3d49ec686be0a80697a09ac3b6fc936968c642df844eee5f5c1d9b89a714"
                }
            }
        }],
        "commandId": create_cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "IndexedMerkleMap",
        "synchronizerId": synchronizer_id
    });

    info!("Submitting HashRequest creation");
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
            "Failed to create HashRequest: HTTP {} - {}",
            create_status,
            create_text
        ));
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    let create_update_id = create_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in create response"))?;

    info!("HashRequest created, updateId: {}", create_update_id);

    // Get the contract ID from the update
    let update_payload = json!({
        "actAs": [party_app_user],
        "updateId": create_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        party_app_user: {
                            "cumulative": [{
                                "identifierFilter": {
                                    "WildcardFilter": {
                                        "value": {
                                            "includeCreatedEventBlob": true
                                        }
                                    }
                                }
                            }]
                        }
                    },
                    "verbose": true
                },
                "transactionShape": "TRANSACTION_SHAPE_ACS_DELTA"
            }
        }
    });

    debug!("Fetching update to get contract ID");
    debug!("Update payload: {}", serde_json::to_string_pretty(&update_payload)?);
    let update_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_status = update_response.status();
    let update_text = update_response.text().await?;

    if !update_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch update: HTTP {} - {}",
            update_status,
            update_text
        ));
    }

    if update_text.is_empty() {
        return Err(anyhow::anyhow!("Empty response when fetching update"));
    }

    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract HashRequest contract ID
    let mut request_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":HashRequest:HashRequest") {
                        request_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let hash_request_cid = request_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find HashRequest contract in create update"))?;

    info!("HashRequest contract ID: {}", hash_request_cid);

    Ok((hash_request_cid, hash_id, create_update_id.to_string(), update_json))
}

/// Create HashRequest and immediately accept it (combined workflow).
/// This creates the HashRequest as the owner, then accepts it as the provider.
/// Returns: (hash_contract_id, hash_id, create_update_id, accept_update_id, create_update_json, accept_update_json)
pub async fn create_and_accept_hash_contract(
    client: &reqwest::Client,
    user_api_url: &str,
    user_jwt: &str,
    party_app_user: &str,
    provider_api_url: &str,
    provider_jwt: &str,
    party_app_provider: &str,
    template_id: &str,
    synchronizer_id: &str,
) -> Result<(String, String, String, String, serde_json::Value, serde_json::Value)> {
    // Step 1: Create HashRequest as owner
    let (request_cid, hash_id, create_update_id, create_update_json) = create_hash_contract(
        client,
        user_api_url,
        user_jwt,
        party_app_user,
        party_app_provider,
        template_id,
        synchronizer_id
    ).await?;

    // Step 2: Accept as provider
    let (hash_contract_id, accept_update_id, accept_update_json) = accept_hash_request(
        client,
        provider_api_url,
        provider_jwt,
        party_app_provider,
        template_id,
        &request_cid,
        synchronizer_id
    ).await?;

    Ok((hash_contract_id, hash_id, create_update_id, accept_update_id, create_update_json, accept_update_json))
}

/// Accept a HashRequest and create the actual Hash contract.
/// This is called by the provider to accept the request.
pub async fn accept_hash_request(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_provider: &str,
    template_id: &str,
    request_contract_id: &str,
    synchronizer_id: &str,
) -> Result<(String, String, serde_json::Value)> {
    let cmdid = format!("accept-hash-request-{}", chrono::Utc::now().timestamp());

    // Extract package ID for HashRequest template
    let package_id = template_id.split(':').next()
        .ok_or_else(|| anyhow::anyhow!("Invalid template_id format"))?;
    let hash_request_template_id = format!("{}:HashRequest:HashRequest", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": hash_request_template_id,
                "contractId": request_contract_id,
                "choice": "Accept",
                "choiceArgument": {
                    "root_time": null
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_provider],
        "readAs": [],
        "workflowId": "IndexedMerkleMap",
        "synchronizerId": synchronizer_id
    });

    info!("Accepting HashRequest");
    debug!("Accept payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to accept HashRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("HashRequest accepted, updateId: {}", update_id);

    // Now we need to get the Hash contract ID from the update
    let update_json = get_update(client, api_url, jwt, party_app_provider, update_id).await?;

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
        .ok_or_else(|| anyhow::anyhow!("Could not find Hash contract in accept update"))?;

    info!("Hash contract ID: {}", hash_contract_id);

    Ok((hash_contract_id, update_id.to_string(), update_json))
}

/// Archive a Hash contract using the built-in Archive choice
pub async fn archive_hash_contract(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    template_id: &str,
    contract_id: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("archive-hash-{}", chrono::Utc::now().timestamp());

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": contract_id,
                "choice": "Archive",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "IndexedMerkleMap",
        "synchronizerId": synchronizer_id
    });

    info!("Archiving Hash contract");

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
            "Failed to archive Hash contract: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_response: serde_json::Value = serde_json::from_str(&text)?;

    let update_id = json_response
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in archive response"))?;

    info!("Hash contract archived, updateId: {}", update_id);

    // Fetch and return the full update
    let update_json = get_update(client, api_url, jwt, party_app_user, update_id).await?;

    Ok((update_id.to_string(), update_json))
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

    // For interface choices (AddMapElement, UpdateMapElement, VerifyInclusion, VerifyExclusion),
    // we need to use the interface ID as the templateId
    let is_interface_choice = matches!(choice_name, "AddMapElement" | "UpdateMapElement" | "VerifyInclusion" | "VerifyExclusion");

    let (effective_template_id, command_key) = if is_interface_choice {
        // Extract package ID from template_id (format: packageId:Module:Template)
        let package_id = template_id.split(':').next()
            .ok_or_else(|| anyhow::anyhow!("Invalid template_id format"))?;
        let interface_id = format!("{}:Silvana:IndexedMerkleMap", package_id);
        (interface_id, "ExerciseCommand")
    } else {
        (template_id.to_string(), "ExerciseCommand")
    };

    let command = json!({
        command_key: {
            "templateId": effective_template_id,
            "contractId": contract_id,
            "choice": choice_name,
            "choiceArgument": choice_argument
        }
    });

    let payload = json!({
        "commands": [command],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "IndexedMerkleMap"
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
                        party_app_user: {
                            "cumulative": [{
                                "identifierFilter": {
                                    "WildcardFilter": {
                                        "value": {
                                            "includeCreatedEventBlob": true
                                        }
                                    }
                                }
                            }]
                        }
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

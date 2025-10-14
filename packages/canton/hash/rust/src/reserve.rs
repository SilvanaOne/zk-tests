use anyhow::Result;
use serde_json::json;
use tracing::{info, debug};

/// Find all Amulet contracts owned by a specific party
async fn find_all_amulets(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
) -> Result<Vec<String>> {
    debug!(party = %party, "Finding all Amulet contracts");

    // Get ledger end offset
    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

    // Query active contracts
    let query = json!({
        "activeAtOffset": offset,
        "filter": {
            "filtersByParty": {
                party: {}
            }
        },
        "verbose": true
    });

    let contracts_url = format!("{}v2/state/active-contracts?limit=500", api_url);
    let contracts: Vec<serde_json::Value> = client
        .post(&contracts_url)
        .bearer_auth(jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Find Amulet contracts (excluding LockedAmulet)
    let mut amulet_cids = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId") {
                        if let Some(template_str) = template_id.as_str() {
                            // Include Splice.Amulet:Amulet but exclude LockedAmulet
                            if template_str.contains("Splice.Amulet:Amulet")
                                && !template_str.contains("LockedAmulet") {
                                if let Some(contract_id) = created.get("contractId") {
                                    let cid = contract_id.as_str().unwrap_or_default().to_string();

                                    // Log the amount if available
                                    if let Some(amount) = created
                                        .pointer("/createArgument/amount/initialAmount")
                                        .and_then(|v| v.as_str())
                                    {
                                        info!(cid = %cid, amount = %amount, "Found Amulet contract");
                                    }

                                    amulet_cids.push(cid);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if amulet_cids.is_empty() {
        return Err(anyhow::anyhow!(
            "No Amulet contracts found for party {}",
            party
        ));
    }

    info!(count = amulet_cids.len(), "Found Amulet contracts");
    Ok(amulet_cids)
}

/// Create a ProofOfReserves contract
async fn create_proof_of_reserves(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    package_id: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("create-proof-of-reserves-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:Reserve:ProofOfReserves", package_id);

    info!("Creating ProofOfReserves contract");

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "prover": party_app_user,
                    "round": {
                        "number": 0
                    },
                    "amount": "1000.0",
                    "guarantors": [{"_1": party_app_user, "_2": "1000.0"}],
                    "observers": []
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "ProofOfReserves",
        "synchronizerId": synchronizer_id
    });

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
            "Failed to create ProofOfReserves: HTTP {} - {}",
            create_status,
            create_text
        ));
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    let create_update_id = create_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in create response"))?;

    info!("ProofOfReserves created, updateId: {}", create_update_id);

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
                "transactionShape": "TRANSACTION_SHAPE_LEDGER_EFFECTS"
            }
        }
    });

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

    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract ProofOfReserves contract ID
    let mut contract_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Reserve:ProofOfReserves") {
                        contract_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let proof_cid = contract_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find ProofOfReserves contract in create update"))?;

    info!("ProofOfReserves contract ID: {}", proof_cid);

    Ok((proof_cid, update_json))
}

/// Exercise the ProveReserve choice with disclosed contracts
async fn exercise_prove_reserve(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    package_id: &str,
    proof_cid: &str,
    amulet_cids: Vec<String>,
    open_round_cid: &str,
    open_round_blob: &str,
    open_round_template_id: &str,
    synchronizer_id: &str,
) -> Result<serde_json::Value> {
    let cmdid = format!("prove-reserve-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:Reserve:ProofOfReserves", package_id);

    info!(amulet_count = amulet_cids.len(), "Exercising ProveReserve choice");

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": proof_cid,
                "choice": "ProveReserve",
                "choiceArgument": {
                    "amuletCids": amulet_cids,
                    "openRoundCid": open_round_cid
                }
            }
        }],
        "disclosedContracts": [
            {
                "contractId": open_round_cid,
                "createdEventBlob": open_round_blob,
                "synchronizerId": synchronizer_id,
                "templateId": open_round_template_id
            }
        ],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "ProofOfReserves",
        "synchronizerId": synchronizer_id
    });

    debug!("ProveReserve payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to exercise ProveReserve: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("ProveReserve executed, updateId: {}", update_id);

    // Fetch the full update
    let update_payload = json!({
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
                "transactionShape": "TRANSACTION_SHAPE_LEDGER_EFFECTS"
            }
        }
    });

    let update_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    Ok(update_json)
}

pub async fn handle_reserve() -> Result<()> {
    info!("Creating Proof of Reserves");

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set in environment"))?;

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Fetch contract blobs context for OpenMiningRound
    info!("Fetching contract context from Scan API...");

    // We need to import context module from billing crate pattern
    // For now, manually fetch the context
    let scan_api_url = std::env::var("SCAN_API_URL")
        .map_err(|_| anyhow::anyhow!("SCAN_API_URL not set in environment"))?;

    // Fetch DSO to get OpenMiningRound
    let dso_url = format!("{}v0/dso", scan_api_url);
    let dso: serde_json::Value = client
        .get(&dso_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let open_round_cid = dso
        .pointer("/latest_mining_round/contract/contract_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not get OpenMiningRound CID"))?
        .to_string();

    let open_round_blob = dso
        .pointer("/latest_mining_round/contract/created_event_blob")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not get OpenMiningRound blob"))?
        .to_string();

    let open_round_template_id = dso
        .pointer("/latest_mining_round/contract/template_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Could not get OpenMiningRound template ID"))?
        .to_string();

    // Extract round number for logging
    let round_number = dso
        .pointer("/latest_mining_round/contract/payload/round/number")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Could not get round number from OpenMiningRound"))?;

    info!("OpenMiningRound CID: {}", open_round_cid);
    info!("Current round number: {}", round_number);

    // Find all amulets owned by PARTY_APP_USER
    let amulet_cids = find_all_amulets(&client, &api_url, &jwt, &party_app_user).await?;

    println!("\nFound {} Amulet contract(s) for {}", amulet_cids.len(), party_app_user);
    for cid in &amulet_cids {
        println!("  - {}", cid);
    }

    // Create ProofOfReserves contract
    let (proof_cid, create_update) = create_proof_of_reserves(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &package_id,
        &synchronizer_id,
    ).await?;

    println!("\n=== Create ProofOfReserves Update ===");
    println!("{}", serde_json::to_string_pretty(&create_update)?);

    println!("\nProofOfReserves contract created: {}", proof_cid);

    // Exercise ProveReserve choice
    let result_json = exercise_prove_reserve(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &package_id,
        &proof_cid,
        amulet_cids.clone(),
        &open_round_cid,
        &open_round_blob,
        &open_round_template_id,
        &synchronizer_id,
    ).await?;

    // Print full update JSON
    println!("\n=== ProveReserve Update ===");
    println!("{}", serde_json::to_string_pretty(&result_json)?);

    // Extract and display results
    println!("\n=== Proof of Reserves Result ===");

    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":Reserve:ProofOfReserves") {
                        if let Some(round_num) = created.pointer("/createArgument/round/number") {
                            println!("Round: {}", round_num);
                        }
                        if let Some(amount) = created.pointer("/createArgument/amount") {
                            println!("Proven Amount: {} CC", amount);
                        }
                        if let Some(guarantors) = created.pointer("/createArgument/guarantors").and_then(|v| v.as_array()) {
                            println!("Guarantors: {}", guarantors.len());
                        }
                        println!("Amulets Verified: {}", amulet_cids.len());
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

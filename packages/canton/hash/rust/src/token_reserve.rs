use anyhow::Result;
use serde_json::json;
use tracing::{info, debug, warn};

/// InstrumentId as defined in Token Standard
#[derive(Debug, Clone)]
struct InstrumentId {
    admin: String,
    id: String,
}

/// Find all Holding contracts owned by a party for a specific instrument
async fn find_holdings(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    instrument_id: &InstrumentId,
    template_id: &str,
) -> Result<Vec<String>> {
    info!("Finding holdings for {} with instrument {}", party, instrument_id.id);

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

    // Query active contracts for the party (we'll filter by template manually)
    let query = json!({
        "activeAtOffset": offset,
        "filter": {
            "filtersByParty": {
                party: {
                    "cumulativeFilter": {
                        "identifierFilter": {
                            "WildcardFilter": {
                                "value": {
                                    "includeCreatedEventBlob": true
                                }
                            }
                        }
                    }
                }
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

    let mut holding_cids = Vec::new();

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    // Verify this is exactly the TestToken template, not TestTokenFactory
                    let contract_template_id = created
                        .get("templateId")
                        .and_then(|v| v.as_str());

                    if contract_template_id != Some(template_id) {
                        continue;
                    }

                    // Check instrument ID in create argument (issuer field was removed)
                    let arg_instrument_id = created
                        .pointer("/createArgument/instrumentId")
                        .and_then(|v| v.as_str());
                    let arg_instrument_admin = created
                        .pointer("/createArgument/instrumentAdmin")
                        .and_then(|v| v.as_str());

                    if arg_instrument_id == Some(&instrument_id.id)
                        && arg_instrument_admin == Some(&instrument_id.admin)
                    {
                        if let Some(cid) = created.get("contractId").and_then(|v| v.as_str()) {
                            let amount = created
                                .pointer("/createArgument/amount")
                                .and_then(|v| v.as_str());
                            info!(
                                cid = %cid,
                                amount = ?amount,
                                "Found Holding contract"
                            );
                            holding_cids.push(cid.to_string());
                        }
                    }
                }
            }
        }
    }

    if holding_cids.is_empty() {
        return Err(anyhow::anyhow!(
            "No Holding contracts found for party {} with instrument {}",
            party,
            instrument_id.id
        ));
    }

    info!(count = holding_cids.len(), "Found Holding contracts");
    Ok(holding_cids)
}

/// Create TokenProofOfReserves contract
async fn create_token_proof_of_reserves(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    prover: &str,
    guarantors: Vec<(String, String)>, // [(party, amount)]
    instrument_id: &InstrumentId,
    package_id: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("create-token-proof-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TokenReserve:TokenProofOfReserves", package_id);

    info!("Creating TokenProofOfReserves contract with {} guarantors", guarantors.len());

    let guarantors_arg: Vec<serde_json::Value> = guarantors
        .iter()
        .map(|(party, amount)| {
            json!({
                "_1": party,
                "_2": amount
            })
        })
        .collect();

    let payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "prover": prover,
                    "round": {
                        "number": 0
                    },
                    "amount": "0.0",
                    "amountHex": "00000000000000000000000000000000",
                    "instrumentId": {
                        "admin": instrument_id.admin,
                        "id": instrument_id.id
                    },
                    "guarantors": guarantors_arg,
                    "observers": []
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [prover],
        "readAs": [],
        "workflowId": "TokenProofOfReserves",
        "synchronizerId": synchronizer_id
    });

    debug!("Create payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to create TokenProofOfReserves: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("TokenProofOfReserves created, updateId: {}", update_id);

    // Fetch the full update
    let update_payload = json!({
        "actAs": [prover],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        prover: {
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

    // Extract contract ID
    let mut contract_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TokenReserve:TokenProofOfReserves") {
                        contract_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = contract_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TokenProofOfReserves contract in create update"))?;

    info!("TokenProofOfReserves contract ID: {}", cid);

    Ok((cid, update_json))
}

/// Exercise ProveTokenReserves choice
async fn exercise_prove_token_reserves(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    prover: &str,
    guarantors: &[(String, String)],
    package_id: &str,
    proof_cid: &str,
    holding_cids: Vec<String>,
    open_round_cid: &str,
    open_round_blob: &str,
    open_round_template_id: &str,
    synchronizer_id: &str,
) -> Result<serde_json::Value> {
    let cmdid = format!("prove-token-reserves-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TokenReserve:TokenProofOfReserves", package_id);

    info!(holding_count = holding_cids.len(), "Exercising ProveTokenReserves choice");

    let guarantor_parties: Vec<String> = guarantors.iter().map(|(p, _)| p.clone()).collect();

    // Build disclosedContracts array with OpenMiningRound
    let disclosed_contracts = vec![
        json!({
            "contractId": open_round_cid,
            "createdEventBlob": open_round_blob,
            "synchronizerId": synchronizer_id,
            "templateId": open_round_template_id
        })
    ];

    // Build actAs list with prover and all guarantors
    let mut act_as_parties = vec![prover.to_string()];
    act_as_parties.extend(guarantor_parties);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": proof_cid,
                "choice": "ProveTokenReserves",
                "choiceArgument": {
                    "holdingCids": holding_cids,
                    "openRoundCid": open_round_cid
                }
            }
        }],
        "disclosedContracts": disclosed_contracts,
        "commandId": cmdid,
        "actAs": act_as_parties,
        "workflowId": "TokenProofOfReserves",
        "synchronizerId": synchronizer_id
    });

    debug!("ProveTokenReserves payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to exercise ProveTokenReserves: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("ProveTokenReserves executed, updateId: {}", update_id);

    // Fetch the full update
    let update_payload = json!({
        "actAs": [prover],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        prover: {
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

pub async fn handle_token_reserve() -> Result<()> {
    info!("Creating Token Proof of Reserves");

    // Load mint.env for token contract IDs
    if let Err(e) = dotenvy::from_filename("mint.env") {
        warn!("Could not load mint.env: {}", e);
    }

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let party_bank = std::env::var("PARTY_BANK")
        .map_err(|_| anyhow::anyhow!("PARTY_BANK not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let token_reserve_package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    // Note: We no longer need a separate test_token_package_id since we use token_reserve_package_id
    // which is the same as HASH_PACKAGE_ID (both TestToken and TokenReserve are in the same package)

    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set in environment"))?;

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    let party_holder = std::env::var("PARTY_HOLDER")
        .map_err(|_| anyhow::anyhow!("PARTY_HOLDER not set in environment"))?;

    // Read instrument ID from mint.env - this is the actual UUID-based instrumentId used for the tokens
    let instrument_id_str = std::env::var("INSTRUMENT_ID")
        .map_err(|_| anyhow::anyhow!("INSTRUMENT_ID not set in environment. Run 'cargo run -- mint' first."))?;

    // Define the instrument we're proving reserves for - use the actual instrumentId from mint.env
    let instrument_id = InstrumentId {
        admin: party_app_user.clone(),
        id: instrument_id_str.clone(),
    };

    println!("\nFinding tokens for instrument: {}", instrument_id_str);
    println!("  HOLDER party: {}", party_holder);
    println!("  BANK party: {}", party_bank);

    let template_id = format!("{}:TestToken:TestToken", token_reserve_package_id);

    // Find HOLDER holdings
    let holder_holdings = find_holdings(
        &client,
        &api_url,
        &jwt,
        &party_holder,
        &instrument_id,
        &template_id,
    ).await?;

    // Find BANK holdings
    let bank_holdings = find_holdings(
        &client,
        &api_url,
        &jwt,
        &party_bank,
        &instrument_id,
        &template_id,
    ).await?;

    // Combine all holdings
    let holder_count = holder_holdings.len();
    let bank_count = bank_holdings.len();
    let mut all_holding_cids = holder_holdings;
    all_holding_cids.extend(bank_holdings);

    println!("Found {} holdings to prove ({} HOLDER + {} BANK)",
        all_holding_cids.len(),
        holder_count,
        bank_count
    );

    // Fetch OpenMiningRound from Scan API
    info!("Fetching OpenMiningRound from Scan API...");
    let scan_api_url = std::env::var("SCAN_API_URL")
        .map_err(|_| anyhow::anyhow!("SCAN_API_URL not set in environment"))?;

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

    let round_number = dso
        .pointer("/latest_mining_round/contract/payload/round/number")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Could not get round number from OpenMiningRound"))?;

    info!("OpenMiningRound CID: {}", open_round_cid);
    info!("Current round number: {}", round_number);

    // Define guarantors with caps (these should match the parties that own the holdings)
    let guarantors = vec![
        (party_holder.clone(), "1000000.0".to_string()),
        (party_bank.clone(), "10000000.0".to_string())
    ];

    // Create TokenProofOfReserves contract
    let (proof_cid, create_update) = create_token_proof_of_reserves(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        guarantors.clone(),
        &instrument_id,
        &token_reserve_package_id,
        &synchronizer_id,
    ).await?;

    println!("\n=== Create TokenProofOfReserves Update ===");
    println!("{}", serde_json::to_string_pretty(&create_update)?);

    println!("\nTokenProofOfReserves contract created: {}", proof_cid);

    // Exercise ProveTokenReserves choice
    let result_json = exercise_prove_token_reserves(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &guarantors,
        &token_reserve_package_id,
        &proof_cid,
        all_holding_cids.clone(),
        &open_round_cid,
        &open_round_blob,
        &open_round_template_id,
        &synchronizer_id,
    ).await?;

    // Print full update JSON
    println!("\n=== ProveTokenReserves Update ===");
    println!("{}", serde_json::to_string_pretty(&result_json)?);

    // Extract and display results
    println!("\n=== Token Proof of Reserves Result ===");

    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TokenReserve:TokenProofOfReserves") {
                        if let Some(round_num) = created.pointer("/createArgument/round/number") {
                            println!("Round: {}", round_num);
                        }
                        if let Some(amount) = created.pointer("/createArgument/amount") {
                            println!("Proven Amount: {} TestToken", amount);
                        }
                        if let Some(instrument) = created.pointer("/createArgument/instrumentId") {
                            println!("Instrument: {}", serde_json::to_string_pretty(&instrument)?);
                        }
                        if let Some(guarantors_val) = created.pointer("/createArgument/guarantors").and_then(|v| v.as_array()) {
                            println!("Guarantors: {}", guarantors_val.len());
                            for g in guarantors_val {
                                if let (Some(party), Some(cap)) = (g.get("_1").and_then(|v| v.as_str()), g.get("_2")) {
                                    println!("  - {} (cap: {})", party, cap);
                                }
                            }
                        }
                        println!("Holdings Verified: {} total", all_holding_cids.len());
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

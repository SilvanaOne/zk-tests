use anyhow::Result;
use serde_json::json;
use tracing::{info, debug, warn};
use crypto_bigint::U256;
use indexed_merkle_map::{Field, IndexedMerkleMap};

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

/// Create TokenProofOfReservesRequest
async fn create_token_proof_request(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    owner: &str,
    provider: &str,
    proof_id: &str,
    guarantors: Vec<(String, String)>, // [(party, amount)]
    instrument_id: &InstrumentId,
    package_id: &str,
    synchronizer_id: &str,
    initial_root: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("create-token-proof-request-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TokenProofOfReservesRequest:TokenProofOfReservesRequest", package_id);

    info!("Creating TokenProofOfReservesRequest with {} guarantors", guarantors.len());

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
                    "owner": owner,
                    "provider": provider,
                    "proofId": proof_id,
                    "round": {
                        "number": 0
                    },
                    "amount": "0.0",
                    "amountHex": "00000000000000000000000000000000",
                    "root": initial_root,
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
        "actAs": [owner],
        "readAs": [],
        "workflowId": format!("TokenProofOfReserves-{}", proof_id),
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

    info!("TokenProofOfReservesRequest created, updateId: {}", update_id);

    // Fetch the full update
    let update_payload = json!({
        "actAs": [owner],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        owner: {
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

    let update_response = client
        .post(&format!("{}v2/updates/update-by-id", api_url))
        .bearer_auth(jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract request contract ID
    let mut request_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TokenProofOfReservesRequest:TokenProofOfReservesRequest") {
                        request_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = request_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TokenProofOfReservesRequest contract in create update"))?;

    info!("TokenProofOfReservesRequest contract ID: {}", cid);

    Ok((cid, update_json))
}

/// Accept TokenProofOfReservesRequest and create the actual TokenProofOfReserves contract
async fn accept_token_proof_request(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    provider: &str,
    package_id: &str,
    request_cid: &str,
    proof_id: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("accept-token-proof-request-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TokenProofOfReservesRequest:TokenProofOfReservesRequest", package_id);

    info!("Accepting TokenProofOfReservesRequest");

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "Accept",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [provider],
        "readAs": [],
        "workflowId": format!("TokenProofOfReserves-{}", proof_id),
        "synchronizerId": synchronizer_id
    });

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
            "Failed to accept TokenProofOfReservesRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("TokenProofOfReservesRequest accepted, updateId: {}", update_id);

    // Fetch the full update to get the TokenProofOfReserves contract ID
    let update_payload = json!({
        "actAs": [provider],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        provider: {
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

    // Extract TokenProofOfReserves contract ID
    let mut proof_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TokenReserve:TokenProofOfReserves") {
                        proof_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = proof_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TokenProofOfReserves contract in accept update"))?;

    info!("TokenProofOfReserves contract ID: {}", cid);

    Ok((cid, update_json))
}

/// Fetch FeaturedAppRight contract ID, blob, and template ID from Scan API
async fn fetch_featured_app_right(
    client: &reqwest::Client,
    scan_api_url: &str,
    provider_party: &str,
    splice_package_id: &str,
) -> Result<Option<(String, String, String)>> {
    info!("Fetching FeaturedAppRight from Scan API for provider: {}", provider_party);

    let featured_url = format!("{}v0/featured-apps", scan_api_url);
    let featured: serde_json::Value = client
        .get(&featured_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let featured_apps = featured
        .get("featured_apps")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Extract provider hint from party (e.g., "app_provider::..." -> "app_provider")
    let provider_hint = provider_party.split("::").next().unwrap_or(provider_party);

    for entry in featured_apps.iter() {
        let provider = entry
            .pointer("/payload/provider")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if provider.contains(provider_hint) {
            let cid = entry
                .pointer("/contract_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            let blob = entry
                .pointer("/created_event_blob")
                .and_then(|v| v.as_str())
                .map(String::from);
            let template_id = entry
                .pointer("/template_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("{}:Splice.Amulet:FeaturedAppRight", splice_package_id));

            if let (Some(c), Some(b)) = (cid, blob) {
                info!("Found FeaturedAppRight CID: {}", c);
                return Ok(Some((c, b, template_id)));
            }
        }
    }

    // If no match by hint, use first available
    if let Some(first) = featured_apps.get(0) {
        let cid = first
            .pointer("/contract_id")
            .and_then(|v| v.as_str())
            .map(String::from);
        let blob = first
            .pointer("/created_event_blob")
            .and_then(|v| v.as_str())
            .map(String::from);
        let template_id = first
            .pointer("/template_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{}:Splice.Amulet:FeaturedAppRight", splice_package_id));

        if let (Some(c), Some(b)) = (cid, blob) {
            info!("Using first FeaturedAppRight CID: {}", c);
            return Ok(Some((c, b, template_id)));
        }
    }

    warn!("No FeaturedAppRight found");
    Ok(None)
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
    proof_id: &str,
    holding_cids: Vec<String>,
    open_round_cid: &str,
    open_round_blob: &str,
    open_round_template_id: &str,
    synchronizer_id: &str,
    witness_json: &serde_json::Value,
    featured_app_right: Option<(String, String, String)>, // (cid, blob, template_id)
) -> Result<serde_json::Value> {
    let cmdid = format!("prove-token-reserves-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TokenReserve:TokenProofOfReserves", package_id);

    info!(holding_count = holding_cids.len(), "Exercising ProveTokenReserves choice");

    let guarantor_parties: Vec<String> = guarantors.iter().map(|(p, _)| p.clone()).collect();

    // Build disclosedContracts array with OpenMiningRound and optionally FeaturedAppRight
    let mut disclosed_contracts = vec![
        json!({
            "contractId": open_round_cid,
            "createdEventBlob": open_round_blob,
            "synchronizerId": synchronizer_id,
            "templateId": open_round_template_id
        })
    ];

    // Add FeaturedAppRight to disclosed contracts if provided
    let featured_app_right_cid = if let Some((cid, blob, template_id)) = featured_app_right {
        disclosed_contracts.push(json!({
            "contractId": cid.clone(),
            "createdEventBlob": blob,
            "synchronizerId": synchronizer_id,
            "templateId": template_id
        }));
        Some(cid)
    } else {
        None
    };

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
                    "openRoundCid": open_round_cid,
                    "witness": witness_json,
                    "featuredAppRightCid": featured_app_right_cid.clone()
                }
            }
        }],
        "disclosedContracts": disclosed_contracts,
        "commandId": cmdid,
        "actAs": act_as_parties,
        "workflowId": format!("TokenProofOfReserves-{}", proof_id),
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

    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PROVIDER not set in environment"))?;

    let party_bank = std::env::var("PARTY_BANK")
        .map_err(|_| anyhow::anyhow!("PARTY_BANK not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set in environment"))?;

    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set in environment"))?;

    let token_reserve_package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let splice_package_id = std::env::var("SPLICE_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("SPLICE_PACKAGE_ID not set in environment"))?;

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

    // Calculate total amount (sum of guarantor caps, will be verified on-chain)
    let total_amount = "1010000.0"; // 10000 + 1000000

    // Generate Merkle witness for (round → amountHex)
    info!("Generating Merkle witness for (round → amountHex)...");
    let mut map = IndexedMerkleMap::new(32);

    // Convert round number to Field (key)
    let round_key = Field::from_u256(U256::from_u64(round_number as u64));

    // Encode amount to fixed-point hex and convert to Field (value)
    let amount_hex = encode_decimal_to_fixed_hex(total_amount)?;
    info!("Amount hex (16-byte): {}", amount_hex);

    // Pad to 32 bytes for Field
    let amount_hex_32 = format!("{:0>64}", amount_hex);
    let amount_field = hex_to_field(&amount_hex_32)?;

    // Generate witness
    let witness = map
        .insert_and_generate_witness(round_key, amount_field, true)?
        .ok_or_else(|| anyhow::anyhow!("Failed to generate Merkle witness"))?;

    info!("Witness generated:");
    info!("  Old root: {}", hex::encode(witness.old_root.as_bytes()));
    info!("  New root: {}", hex::encode(witness.new_root.as_bytes()));
    info!("  Key (round): {}", hex::encode(witness.key.as_bytes()));
    info!("  Value (amountHex): {}", hex::encode(witness.value.as_bytes()));

    // Convert witness to Daml JSON
    let witness_json = convert_witness_to_daml_json(&witness)?;

    // Get the old root (initial empty map root) for the contract
    let initial_root = hex::encode(witness.old_root.as_bytes());

    // Generate unique proof ID (UUID)
    let proof_id = uuid::Uuid::new_v4().to_string();
    info!("Generated proof ID: {}", proof_id);

    // Step 1: Create TokenProofOfReservesRequest (owner proposes)
    info!("Creating TokenProofOfReservesRequest via propose-accept workflow...");
    let (request_cid, create_update) = create_token_proof_request(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_app_provider,
        &proof_id,
        guarantors.clone(),
        &instrument_id,
        &token_reserve_package_id,
        &synchronizer_id,
        &initial_root,
    ).await?;

    println!("\n=== TokenProofOfReservesRequest Creation ===");
    println!("{}", serde_json::to_string_pretty(&create_update)?);
    println!("\nRequest contract ID: {}", request_cid);

    // Step 2: Accept the request (provider accepts)
    let (proof_cid, accept_update) = accept_token_proof_request(
        &client,
        &provider_api_url,
        &provider_jwt,
        &party_app_provider,
        &token_reserve_package_id,
        &request_cid,
        &proof_id,
        &synchronizer_id,
    ).await?;

    println!("\n=== TokenProofOfReservesRequest Acceptance ===");
    println!("{}", serde_json::to_string_pretty(&accept_update)?);
    println!("\nTokenProofOfReserves contract created: {}", proof_cid);

    // Fetch FeaturedAppRight for activity marker
    info!("Fetching FeaturedAppRight for provider: {}", party_app_provider);
    let featured_app_right = fetch_featured_app_right(
        &client,
        &scan_api_url,
        &party_app_provider,
        &splice_package_id,
    ).await?;

    if let Some((ref cid, _, _)) = featured_app_right {
        println!("\nFeaturedAppRight CID: {}", cid);
    } else {
        println!("\nNo FeaturedAppRight found - activity marker will not be created");
    }

    // Exercise ProveTokenReserves choice
    let result_json = exercise_prove_token_reserves(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &guarantors,
        &token_reserve_package_id,
        &proof_cid,
        &proof_id,
        all_holding_cids.clone(),
        &open_round_cid,
        &open_round_blob,
        &open_round_template_id,
        &synchronizer_id,
        &witness_json,
        featured_app_right,
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

/// Convert hex string to Field (32-byte)
fn hex_to_field(hex_str: &str) -> Result<Field> {
    // Remove 0x prefix if present
    let hex_clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);

    // Pad to 64 chars (32 bytes) if shorter
    let hex_padded = if hex_clean.len() < 64 {
        format!("{:0>64}", hex_clean)
    } else if hex_clean.len() > 64 {
        return Err(anyhow::anyhow!("Hex string too long: {} chars", hex_clean.len()));
    } else {
        hex_clean.to_string()
    };

    let bytes = hex::decode(&hex_padded)?;
    let mut field_bytes = [0u8; 32];
    field_bytes.copy_from_slice(&bytes);

    Ok(Field::from_bytes(field_bytes))
}

/// Encode decimal to fixed-point hex (16 bytes)
/// Using scale=10 (10 decimal places)
fn encode_decimal_to_fixed_hex(decimal_str: &str) -> Result<String> {
    // Parse decimal string
    let parts: Vec<&str> = decimal_str.split('.').collect();
    let (int_part, frac_part) = match parts.len() {
        1 => (parts[0], ""),
        2 => (parts[0], parts[1]),
        _ => return Err(anyhow::anyhow!("Invalid decimal format")),
    };

    // Pad or truncate fractional part to exactly 10 digits
    let frac_padded = if frac_part.len() < 10 {
        format!("{:0<10}", frac_part)
    } else {
        frac_part[..10].to_string()
    };

    // Combine integer and fractional parts
    let combined = format!("{}{}", int_part, frac_padded);

    // Parse as u128
    let value: u128 = combined.parse()?;

    // Convert to 16-byte big-endian
    let bytes = value.to_be_bytes();

    Ok(hex::encode(&bytes))
}

/// Convert witness to Daml JSON format
fn convert_witness_to_daml_json(witness: &indexed_merkle_map::InsertWitness) -> Result<serde_json::Value> {
    let field_to_hex = |f: &Field| hex::encode(f.as_bytes());
    let hash_to_hex = |h: &indexed_merkle_map::Hash| hex::encode(h.as_bytes());

    let leaf_to_json = |leaf: &indexed_merkle_map::Leaf| {
        json!({
            "key": field_to_hex(&leaf.key),
            "value": field_to_hex(&leaf.value),
            "nextKey": field_to_hex(&leaf.next_key),
            "index": leaf.index
        })
    };

    let proof_to_json = |proof: &indexed_merkle_map::MerkleProof| {
        json!({
            "siblings": proof.siblings.iter().map(hash_to_hex).collect::<Vec<_>>(),
            "pathIndices": proof.path_indices.clone()
        })
    };

    Ok(json!({
        "oldRoot": hash_to_hex(&witness.old_root),
        "newRoot": hash_to_hex(&witness.new_root),
        "key": field_to_hex(&witness.key),
        "value": field_to_hex(&witness.value),
        "newLeafIndex": witness.new_leaf_index,
        "treeLength": witness.tree_length,
        "lowLeaf": leaf_to_json(&witness.non_membership_proof.low_leaf),
        "lowLeafProof": proof_to_json(&witness.low_leaf_proof_before),
        "updatedLowLeaf": leaf_to_json(&witness.updated_low_leaf),
        "newLeaf": leaf_to_json(&witness.new_leaf),
        "newLeafProofAfter": proof_to_json(&witness.new_leaf_proof_after)
    }))
}

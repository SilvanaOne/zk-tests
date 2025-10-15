//! Claim AppRewardCoupons and mint Canton Coin

use crate::context::ContractBlobsContext;
use crate::url::create_client_with_localhost_resolution;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, info};

/// Handle the claim command - find and claim all AppRewardCoupons
pub async fn handle_claim() -> Result<()> {
    info!("Starting AppRewardCoupon claim process");

    // Load environment variables
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .context("APP_PROVIDER_JWT not set")?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .context("APP_PROVIDER_API_URL not set")?;
    let scan_api_url = std::env::var("SCAN_API_URL")
        .context("SCAN_API_URL not set")?;
    let party_provider = std::env::var("PARTY_APP_PROVIDER")
        .context("PARTY_APP_PROVIDER not set")?;
    let splice_package_id = std::env::var("SPLICE_PACKAGE_ID")
        .context("SPLICE_PACKAGE_ID not set")?;

    info!(
        provider = %party_provider,
        api_url = %provider_api_url,
        "Loaded configuration"
    );

    // Build HTTP client
    let client = create_client_with_localhost_resolution()?;

    // Fetch contract blobs context (AmuletRules CID, OpenMiningRound CID, etc.)
    println!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    info!(
        amulet_rules_cid = %context.amulet_rules_cid,
        open_mining_round_cid = %context.open_mining_round_cid,
        "Fetched contract context"
    );

    // Step 1: Find all AppRewardCoupon contracts for the provider
    println!("Searching for AppRewardCoupon contracts...");
    let coupons = find_app_reward_coupons(
        &client,
        &provider_api_url,
        &provider_jwt,
        &party_provider,
        &splice_package_id,
    ).await?;

    if coupons.is_empty() {
        println!("✅ No AppRewardCoupon contracts found - nothing to claim");
        return Ok(());
    }

    println!("Found {} AppRewardCoupon(s) to claim:", coupons.len());
    let mut total_amount = 0.0;
    for coupon in &coupons {
        println!("  • Round {}: {:.10} CC (CID: {})",
            coupon.round, coupon.amount, &coupon.contract_id[..16]);
        total_amount += coupon.amount;
    }
    println!("  Total: {:.10} CC\n", total_amount);

    // Step 2: Find IssuingMiningRound contracts for each coupon's round
    println!("Fetching IssuingMiningRound contracts...");
    let rounds: Vec<i64> = coupons.iter().map(|c| c.round).collect();
    let issuing_rounds = find_issuing_rounds(
        &client,
        &scan_api_url,
        &provider_jwt,
        &party_provider,
        &splice_package_id,
        &rounds,
    ).await?;

    info!(
        found_rounds = issuing_rounds.len(),
        needed_rounds = rounds.len(),
        "Fetched IssuingMiningRound contracts"
    );

    // Separate coupons with available IssuingMiningRounds vs those without
    let coupons_with_rounds: Vec<_> = coupons.iter()
        .filter(|c| issuing_rounds.contains_key(&c.round))
        .cloned()
        .collect();

    let coupons_without_rounds: Vec<_> = coupons.iter()
        .filter(|c| !issuing_rounds.contains_key(&c.round))
        .collect();

    if !coupons_without_rounds.is_empty() {
        let missing_rounds: Vec<_> = coupons_without_rounds.iter()
            .map(|c| c.round)
            .collect();
        let missing_amount: f64 = coupons_without_rounds.iter()
            .map(|c| c.amount)
            .sum();

        println!("\n⚠️  Skipping {} coupon(s) from rounds {:?} (no IssuingMiningRound found)",
                 coupons_without_rounds.len(), missing_rounds);
        println!("   Amount: {:.10} CC", missing_amount);
    }

    if coupons_with_rounds.is_empty() {
        return Err(anyhow!(
            "No rewards can be claimed. All coupons require IssuingMiningRounds that do not exist."
        ));
    }

    let total_attempt_amount: f64 = coupons_with_rounds.iter().map(|c| c.amount).sum();
    println!("\n✅ Attempting to claim {} coupon(s) ({:.10} CC)",
             coupons_with_rounds.len(), total_attempt_amount);

    // Step 3: Build and execute the transfer to claim rewards
    println!("Executing AmuletRules_Transfer to claim rewards...");
    let result = execute_claim_transfer(
        &client,
        &provider_api_url,
        &provider_jwt,
        &party_provider,
        &context,
        &coupons_with_rounds,
        &issuing_rounds,
        &splice_package_id,
    ).await;

    // Handle the result - show detailed error if claim fails
    match result {
        Ok(claim_result) => {
            // Display success results
            println!("\n✅ Successfully claimed rewards!");
            println!("════════════════════════════════════════");
            println!("Transaction Details:");
            println!("  Update ID: {}", claim_result.update_id);
            println!("  Claimed Amount: {:.10} CC", total_attempt_amount);
            println!("  Coupons Claimed: {}", coupons_with_rounds.len());

            if let Some(amulet_cids) = claim_result.created_amulets {
                println!("  Created Amulets: {}", amulet_cids.len());
                for (idx, cid) in amulet_cids.iter().enumerate() {
                    println!("    {}. {}", idx + 1, cid);
                }
            }
            Ok(())
        }
        Err(e) => {
            // Show detailed error information with full debug output
            println!("\n❌ Failed to claim rewards");
            println!("════════════════════════════════════════");

            // Show both the error message and the full debug representation
            let error_msg = format!("{:#}", e);
            let error_debug = format!("{:?}", e);

            // Check for specific error types and provide helpful messages
            if error_msg.contains("deadline-not-exceeded") || error_msg.contains("opensAt") {
                println!("⏰ Timing Issue: IssuingMiningRound has not opened yet");

                // Try to extract the open time for each round
                for coupon in &coupons_with_rounds {
                    println!("   Round {}: Waiting for IssuingMiningRound to open", coupon.round);
                }
            } else if error_msg.contains("TransferContext did not contain issuing mining round") {
                println!("🔍 Context Issue: Required IssuingMiningRound not in transfer context");
            }

            println!("\nFull Error Details:");
            println!("{}", error_msg);
            println!("\nFull Error Debug:");
            println!("{}", error_debug);

            Err(e)
        }
    }
}

/// Represents an AppRewardCoupon contract
#[derive(Debug, Clone)]
struct RewardCoupon {
    contract_id: String,
    amount: f64,
    round: i64,
}

/// Represents an IssuingMiningRound contract with its blob
#[derive(Debug, Clone)]
struct IssuingRound {
    contract_id: String,
    created_event_blob: String,
}

/// Result of claim transfer execution
#[derive(Debug)]
struct ClaimResult {
    update_id: String,
    created_amulets: Option<Vec<String>>,
}

/// Find all AppRewardCoupon contracts for the provider
async fn find_app_reward_coupons(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    provider_party: &str,
    _package_id: &str,
) -> Result<Vec<RewardCoupon>> {
    let url = format!("{}v2/state/active-contracts", api_url);

    // Get current ledger end
    let ledger_end_url = format!("{}v2/state/ledger-end", api_url);
    let ledger_end_resp: Value = client
        .get(&ledger_end_url)
        .header("Authorization", format!("Bearer {}", jwt))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset_str = if let Some(s) = ledger_end_resp.get("offset").and_then(|v| v.as_str()) {
        s.to_string()
    } else if let Some(n) = ledger_end_resp.get("offset").and_then(|v| v.as_u64()) {
        n.to_string()
    } else {
        return Err(anyhow!("Unable to get ledger end offset"));
    };

    // Query for ALL contracts for this party (no template filter)
    // We'll filter by template ID in Rust code, matching the Makefile pattern
    // Note: We need to build the filtersByParty object dynamically with the party as a key
    let mut filters_by_party = serde_json::Map::new();
    filters_by_party.insert(provider_party.to_string(), json!({}));

    let request_body = json!({
        "activeAtOffset": offset_str.parse::<i64>()?,
        "filter": {
            "filtersByParty": filters_by_party
        },
        "verbose": true
    });

    debug!(party = %provider_party, offset = %offset_str, "Querying for all contracts");
    debug!(request = ?request_body, "Request body");

    let response: Value = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut coupons = Vec::new();

    // The response is an array, not an object!
    let contracts_array = response.as_array()
        .ok_or_else(|| anyhow!("Expected array response from active-contracts API"))?;

    debug!(count = contracts_array.len(), "Received contracts from API");

    for value in contracts_array.iter() {
            if let Some(contract_entry) = value.get("contractEntry") {
                if let Some(js_contract) = contract_entry.get("JsActiveContract") {
                    let created_event = js_contract.get("createdEvent")
                        .ok_or_else(|| anyhow!("Missing createdEvent"))?;

                    // Filter by template ID - only process AppRewardCoupon contracts
                    let template_id_str = created_event.get("templateId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    debug!(template_id = %template_id_str, "Found contract");

                    if !template_id_str.contains("AppRewardCoupon") {
                        continue;  // Skip non-AppRewardCoupon contracts
                    }

                    debug!(template_id = %template_id_str, "Found AppRewardCoupon!");

                    let contract_id = created_event.get("contractId")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow!("Missing contractId"))?
                        .to_string();

                    let args = created_event.get("createArguments")
                        .or_else(|| created_event.get("createArgument"))
                        .ok_or_else(|| anyhow!("Missing create arguments"))?;

                    let amount = args.get("amount")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or_else(|| anyhow!("Missing or invalid amount"))?;

                    let round_obj = args.get("round")
                        .ok_or_else(|| anyhow!("Missing round"))?;
                    let round = round_obj.get("number")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i64>().ok())
                        .ok_or_else(|| anyhow!("Missing or invalid round number"))?;

                    coupons.push(RewardCoupon {
                        contract_id,
                        amount,
                        round,
                    });
                }
            }
    }

    debug!(count = coupons.len(), "Found AppRewardCoupons");
    Ok(coupons)
}

/// Find IssuingMiningRound contracts for the given round numbers using Scan API
async fn find_issuing_rounds(
    client: &reqwest::Client,
    scan_api_url: &str,
    _jwt: &str,
    _party: &str,
    _package_id: &str,
    rounds: &[i64],
) -> Result<HashMap<i64, IssuingRound>> {
    // Query Scan API for open and issuing mining rounds
    let url = format!("{}v0/open-and-issuing-mining-rounds", scan_api_url);

    let request_body = json!({
        "cached_open_mining_round_contract_ids": [],
        "cached_issuing_round_contract_ids": []
    });

    debug!("Querying Scan API for IssuingMiningRounds");

    let response: Value = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut issuing_rounds = HashMap::new();
    let mut all_found_rounds = Vec::new();
    let mut all_contracts_json = Vec::new();

    // Parse issuing_mining_rounds object (keyed by contract ID)
    if let Some(issuing_map) = response.get("issuing_mining_rounds").and_then(|v| v.as_object()) {
        for (_cid, value) in issuing_map.iter() {
            if let Some(contract) = value.get("contract") {
                let contract_id = contract.get("contract_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing contract_id"))?
                    .to_string();

                let blob = contract.get("created_event_blob")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing created_event_blob"))?
                    .to_string();

                let round = contract.pointer("/payload/round/number")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or_else(|| anyhow!("Missing or invalid round number"))?;

                // Track all found rounds and their full JSON
                all_found_rounds.push(round);
                all_contracts_json.push(json!({
                    "round": round,
                    "contract_id": contract_id,
                    "template_id": contract.get("template_id"),
                    "payload": contract.get("payload"),
                }));

                // Only include rounds we need
                if rounds.contains(&round) {
                    issuing_rounds.insert(round, IssuingRound {
                        contract_id,
                        created_event_blob: blob,
                    });
                }
            }
        }
    }

    // Debug: Show all available IssuingMiningRounds
    if !all_found_rounds.is_empty() {
        all_found_rounds.sort();
        println!("📋 Available IssuingMiningRound contracts: {:?}", all_found_rounds);
        println!("   (Needed rounds: {:?})", rounds);
        println!("\n📄 Full IssuingMiningRound contracts JSON:");
        println!("{}", serde_json::to_string_pretty(&all_contracts_json).unwrap_or_else(|_| "{}".to_string()));
    } else {
        println!("📋 No IssuingMiningRound contracts found on ledger");
        println!("   Rewards can only be claimed when their round is in the issuing phase.");
        println!("   Check the Canton Network UI to see current round phases.");
    }

    debug!(count = issuing_rounds.len(), "Found IssuingMiningRounds");
    Ok(issuing_rounds)
}

/// Execute AmuletRules_Transfer to claim rewards
async fn execute_claim_transfer(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    provider_party: &str,
    context: &ContractBlobsContext,
    coupons: &[RewardCoupon],
    issuing_rounds: &HashMap<i64, IssuingRound>,
    splice_package_id: &str,
) -> Result<ClaimResult> {
    let url = format!("{}v2/commands/submit-and-wait", api_url);

    // Build transfer inputs - one for each AppRewardCoupon
    let mut transfer_inputs = Vec::new();
    for coupon in coupons {
        transfer_inputs.push(json!({
            "tag": "InputAppRewardCoupon",
            "value": coupon.contract_id
        }));
    }

    // Calculate total amount to receive
    let total_amount: f64 = coupons.iter().map(|c| c.amount).sum();

    // Build transfer outputs - single output to provider
    let transfer_outputs = vec![json!({
        "receiver": provider_party,
        "amount": format!("{}", total_amount),
        "receiverFeeRatio": "0.0"
    })];

    // Build transfer object
    let transfer = json!({
        "sender": provider_party,
        "provider": provider_party,
        "inputs": transfer_inputs,
        "outputs": transfer_outputs,
        "beneficiaries": null
    });

    // Build issuingMiningRounds map from Round object to contract ID
    // Daml Maps are represented as arrays of [key, value] pairs in JSON API
    // Round is a record type: {number: string}
    let mut issuing_rounds_array = Vec::new();
    let mut missing_rounds = Vec::new();
    for coupon in coupons {
        if let Some(issuing_round) = issuing_rounds.get(&coupon.round) {
            issuing_rounds_array.push(json!([
                {"number": coupon.round.to_string()},
                issuing_round.contract_id
            ]));
        } else {
            missing_rounds.push(coupon.round);
        }
    }

    // Build context: TransferContext has direct fields, not nested amuletRules
    // Note: Daml Maps are represented as arrays of [key, value] pairs in JSON API
    let transfer_context = json!({
        "openMiningRound": context.open_mining_round_cid,
        "issuingMiningRounds": issuing_rounds_array,
        "validatorRights": [],  // Empty Daml Map = empty array
        "featuredAppRight": context.featured_app_right_cid
    });

    // Build disclosed contracts - include all referenced contracts
    let mut disclosed_contracts = Vec::new();

    // Add AmuletRules
    disclosed_contracts.push(json!({
        "contractId": context.amulet_rules_cid,
        "contractIdActual": context.amulet_rules_cid,
        "blob": context.amulet_rules_blob,
        "createdEventBlob": context.amulet_rules_blob,
        "synchronizerId": context.synchronizer_id,
        "templateId": context.amulet_rules_template_id
    }));

    // Add OpenMiningRound
    disclosed_contracts.push(json!({
        "contractId": context.open_mining_round_cid,
        "contractIdActual": context.open_mining_round_cid,
        "blob": context.open_mining_round_blob,
        "createdEventBlob": context.open_mining_round_blob,
        "synchronizerId": context.synchronizer_id,
        "templateId": context.open_mining_round_template_id
    }));

    // Add FeaturedAppRight
    disclosed_contracts.push(json!({
        "contractId": context.featured_app_right_cid,
        "contractIdActual": context.featured_app_right_cid,
        "blob": context.featured_app_right_blob,
        "createdEventBlob": context.featured_app_right_blob,
        "synchronizerId": context.synchronizer_id,
        "templateId": context.featured_app_right_template_id
    }));

    // Add IssuingMiningRounds - deduplicate by contract_id to avoid duplicate disclosed contracts
    let mut added_issuing_rounds = std::collections::HashSet::new();
    for coupon in coupons {
        if let Some(issuing_round) = issuing_rounds.get(&coupon.round) {
            // Only add if we haven't already added this contract ID
            if added_issuing_rounds.insert(issuing_round.contract_id.clone()) {
                disclosed_contracts.push(json!({
                    "contractId": issuing_round.contract_id,
                    "contractIdActual": issuing_round.contract_id,
                    "blob": issuing_round.created_event_blob,
                    "createdEventBlob": issuing_round.created_event_blob,
                    "synchronizerId": context.synchronizer_id,
                    "templateId": format!("{}:Splice.Round:IssuingMiningRound", splice_package_id)
                }));
            }
        }
    }

    // Note: missing_rounds handling is now done in the main claim() function
    // which filters coupons before calling this function

    // Build exercise command with proper ExerciseCommand wrapper
    let command = json!({
        "ExerciseCommand": {
            "templateId": context.amulet_rules_template_id,
            "contractId": context.amulet_rules_cid,
            "choice": "AmuletRules_Transfer",
            "choiceArgument": {
                "transfer": transfer,
                "context": transfer_context,
                "expectedDso": context.dso_party
            }
        }
    });

    let request_body = json!({
        "commands": [command],
        "commandId": format!("claim-{}", uuid::Uuid::new_v4()),
        "actAs": [provider_party],
        "workflowId": format!("claim-rewards-{}", chrono::Utc::now().timestamp()),
        "disclosedContracts": disclosed_contracts,
        "readAs": []
    });

    debug!("Submitting claim transfer command");
    debug!(payload = ?request_body, "Request payload");

    let http_response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = http_response.status();
    if !status.is_success() {
        let error_text = http_response.text().await?;

        // Always return the full HTTP error
        return Err(anyhow!("HTTP {} - {}", status, error_text));
    }

    let response: Value = http_response.json().await?;

    // Extract update ID from response
    let update_id = response["updateId"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing updateId in response"))?
        .to_string();

    // Try to extract created Amulet contract IDs from events
    let created_amulets = extract_created_amulets(&response);

    info!(update_id = %update_id, "Claim transfer submitted successfully");

    Ok(ClaimResult {
        update_id,
        created_amulets,
    })
}

/// Extract created Amulet contract IDs from transaction response
fn extract_created_amulets(response: &Value) -> Option<Vec<String>> {
    let mut amulet_cids = Vec::new();

    // Try to find created events in the response
    if let Some(created_events) = response.pointer("/exerciseResult/events")
        .and_then(|v| v.as_array())
    {
        for event in created_events {
            if let Some(created) = event.get("created") {
                if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                    // Check if it's an Amulet contract
                    if template_id.contains(":Splice.Amulet:Amulet") {
                        if let Some(contract_id) = created.get("contractId").and_then(|v| v.as_str()) {
                            amulet_cids.push(contract_id.to_string());
                        }
                    }
                }
            }
        }
    }

    if amulet_cids.is_empty() {
        None
    } else {
        Some(amulet_cids)
    }
}

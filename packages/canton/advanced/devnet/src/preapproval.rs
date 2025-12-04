//! TransferPreapproval commands for external party transfers.
//!
//! Implements:
//! - request: Create TransferPreapprovalProposal (interactive submission)
//! - accept: Accept pending proposals (provider action, standard submission)
//! - cancel: Cancel TransferPreapproval contracts (standard submission)
//! - transfer: Transfer via TransferFactory_Transfer (interactive submission)

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::json;
use tracing::info;

use crate::context::{
    create_client, get_amulet_rules, get_dso_party, get_external_party_amulet_rules,
    get_featured_app_right_for_provider, get_open_mining_round, ContractInfo,
};
use crate::interactive::{submit_interactive, SubmissionResult};
use crate::list::find_amulets;
use crate::signing::parse_base58_private_key;

/// Default provider for all preapproval operations
pub const DEFAULT_PARTY_PROVIDER: &str =
    "orderbook-operator-1::122034faf8f4af71d107a42441f8bc90cabfd63ab4386fc7f17d15d6e3b01c5bd2ae";

/// Get the party provider from environment or use default
fn get_party_provider() -> String {
    std::env::var("PARTY_PROVIDER").unwrap_or_else(|_| DEFAULT_PARTY_PROVIDER.to_string())
}

/// Request TransferPreapproval for an external party.
///
/// Creates a TransferPreapprovalProposal using interactive submission where:
/// - `receiver` is the external party (party_id)
/// - `provider` is PARTY_PROVIDER
///
/// The external party signs with their Ed25519 private key.
pub async fn handle_request_preapproval(party_id: String, private_key: String) -> Result<String> {
    let ledger_api_url =
        std::env::var("LEDGER_API_URL").map_err(|_| anyhow!("LEDGER_API_URL not set"))?;
    let scan_api_url =
        std::env::var("SCAN_API_URL").map_err(|_| anyhow!("SCAN_API_URL not set"))?;
    let synchronizer_id =
        std::env::var("SYNCHRONIZER_ID").map_err(|_| anyhow!("SYNCHRONIZER_ID not set"))?;
    let jwt_provider =
        std::env::var("JWT_PROVIDER").map_err(|_| anyhow!("JWT_PROVIDER not set"))?;

    let provider = get_party_provider();
    let dso_party = get_dso_party(&scan_api_url).await?;

    // Parse Base58 private key to get 32-byte seed
    let seed = parse_base58_private_key(&private_key)?;

    info!(
        party_id = %party_id,
        provider = %provider,
        dso_party = %dso_party,
        "Creating TransferPreapprovalProposal for external party"
    );

    println!("\nCreating TransferPreapprovalProposal...");
    println!("  Receiver: {}", party_id);
    println!("  Provider: {}", provider);

    let client = create_client()?;

    // Build CreateCommand for TransferPreapprovalProposal
    let commands = vec![json!({
        "CreateCommand": {
            "templateId": "#splice-wallet:Splice.Wallet.TransferPreapproval:TransferPreapprovalProposal",
            "createArguments": {
                "provider": provider,
                "receiver": party_id,
                "expectedDso": dso_party
            }
        }
    })];

    // No disclosures needed for CreateCommand
    let disclosed_contracts: Vec<serde_json::Value> = vec![];

    // Submit via interactive submission (prepare → sign → execute)
    let result = submit_interactive(
        &client,
        &ledger_api_url,
        &jwt_provider,
        &party_id,
        &synchronizer_id,
        &seed,
        commands,
        disclosed_contracts,
    )
    .await?;

    println!("\n✓ TransferPreapprovalProposal created!");
    println!("  Update ID: {}", result.update_id);
    println!("\nNext: Run 'preapproval accept' to accept this proposal.");

    Ok(result.update_id)
}

/// Accept all pending TransferPreapprovalProposal contracts.
///
/// Provider accepts proposals using standard submit-and-wait submission.
pub async fn handle_accept_preapprovals() -> Result<Vec<String>> {
    let ledger_api_url =
        std::env::var("LEDGER_API_URL").map_err(|_| anyhow!("LEDGER_API_URL not set"))?;
    let scan_api_url =
        std::env::var("SCAN_API_URL").map_err(|_| anyhow!("SCAN_API_URL not set"))?;
    let synchronizer_id =
        std::env::var("SYNCHRONIZER_ID").map_err(|_| anyhow!("SYNCHRONIZER_ID not set"))?;
    let jwt_provider =
        std::env::var("JWT_PROVIDER").map_err(|_| anyhow!("JWT_PROVIDER not set"))?;

    let provider = get_party_provider();

    info!(provider = %provider, "Finding pending TransferPreapprovalProposals...");

    let client = create_client()?;

    // Query active contracts for provider
    let offset = get_ledger_end(&client, &ledger_api_url, &jwt_provider).await?;
    let contracts =
        query_active_contracts(&client, &ledger_api_url, &jwt_provider, &provider, &offset).await?;

    // Filter for TransferPreapprovalProposal contracts
    let proposals: Vec<_> = contracts
        .iter()
        .filter(|c| {
            c.get("templateId")
                .and_then(|v| v.as_str())
                .map(|t| t.contains("TransferPreapprovalProposal"))
                .unwrap_or(false)
        })
        .collect();

    if proposals.is_empty() {
        println!("No pending TransferPreapprovalProposals found.");
        return Ok(vec![]);
    }

    println!("\nFound {} pending proposals:", proposals.len());
    for proposal in &proposals {
        let receiver = proposal
            .pointer("/payload/receiver")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let cid = proposal
            .get("contractId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("  - Receiver: {}", receiver);
        println!("    Contract ID: {}", cid);
    }
    println!();

    // Fetch context contracts
    let amulet_rules = get_amulet_rules(&scan_api_url).await?;
    let open_mining_round = get_open_mining_round(&scan_api_url).await?;

    // Calculate expiry (30 days from now)
    let expires_at = (Utc::now() + chrono::Duration::days(30))
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string();

    let mut accepted = vec![];
    let url = format!("{}/commands/submit-and-wait", ledger_api_url);
    let num_proposals = proposals.len();

    for proposal in &proposals {
        let proposal_cid = proposal
            .get("contractId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing contractId"))?;
        let receiver = proposal
            .pointer("/payload/receiver")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!(cid = %proposal_cid, receiver = %receiver, "Accepting proposal...");

        // Fetch provider's amulets for fee inputs
        let provider_amulets = find_amulets(&client, &ledger_api_url, &jwt_provider, &provider).await?;
        let inputs: Vec<serde_json::Value> = provider_amulets
            .iter()
            .map(|a| {
                json!({
                    "tag": "InputAmulet",
                    "value": a.contract_id
                })
            })
            .collect();

        let accept_payload = json!({
            "commands": [{
                "ExerciseCommand": {
                    "templateId": "#splice-wallet:Splice.Wallet.TransferPreapproval:TransferPreapprovalProposal",
                    "contractId": proposal_cid,
                    "choice": "TransferPreapprovalProposal_Accept",
                    "choiceArgument": {
                        "context": {
                            "amuletRules": amulet_rules.contract_id,
                            "context": {
                                "openMiningRound": open_mining_round.contract_id,
                                "issuingMiningRounds": [],
                                "validatorRights": [],
                                "featuredAppRight": serde_json::Value::Null
                            }
                        },
                        "inputs": inputs,
                        "expiresAt": expires_at
                    }
                }
            }],
            "disclosedContracts": [
                {
                    "contractId": amulet_rules.contract_id,
                    "createdEventBlob": amulet_rules.created_event_blob,
                    "synchronizerId": synchronizer_id,
                    "templateId": amulet_rules.template_id
                },
                {
                    "contractId": open_mining_round.contract_id,
                    "createdEventBlob": open_mining_round.created_event_blob,
                    "synchronizerId": synchronizer_id,
                    "templateId": open_mining_round.template_id
                }
            ],
            "commandId": format!("accept-preapproval-{}", Utc::now().timestamp_millis()),
            "actAs": [&provider],
            "readAs": []
        });

        let response = client
            .post(&url)
            .bearer_auth(&jwt_provider)
            .json(&accept_payload)
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await?;

        if status.is_success() {
            let update_id = body
                .get("updateId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!("✓ Accepted proposal for receiver: {}", receiver);
            println!("  Update ID: {}", update_id);
            accepted.push(update_id.to_string());
        } else {
            println!("✗ Failed to accept proposal for receiver: {}", receiver);
            println!("  Error: {:?}", body);
        }
    }

    println!(
        "\nAccepted {} of {} proposals.",
        accepted.len(),
        num_proposals
    );
    Ok(accepted)
}

/// Cancel all TransferPreapproval contracts for a party.
pub async fn handle_cancel_preapprovals(
    party_id: Option<String>,
    jwt: Option<String>,
) -> Result<Vec<String>> {
    let ledger_api_url =
        std::env::var("LEDGER_API_URL").map_err(|_| anyhow!("LEDGER_API_URL not set"))?;

    // Use provided party_id or fall back to PARTY_PROVIDER
    let party = party_id.unwrap_or_else(get_party_provider);

    // Use provided JWT or fall back to JWT_PROVIDER
    let jwt = jwt.unwrap_or_else(|| {
        std::env::var("JWT_PROVIDER").unwrap_or_default()
    });

    if jwt.is_empty() {
        return Err(anyhow!("JWT not provided and JWT_PROVIDER not set"));
    }

    info!(party = %party, "Finding TransferPreapproval contracts...");

    let client = create_client()?;

    // Query active contracts
    let offset = get_ledger_end(&client, &ledger_api_url, &jwt).await?;
    let contracts =
        query_active_contracts(&client, &ledger_api_url, &jwt, &party, &offset).await?;

    // Filter for TransferPreapproval (not Proposal)
    let preapprovals: Vec<_> = contracts
        .iter()
        .filter(|c| {
            let template_id = c.get("templateId").and_then(|v| v.as_str()).unwrap_or("");
            template_id.contains("TransferPreapproval") && !template_id.contains("Proposal")
        })
        .collect();

    if preapprovals.is_empty() {
        println!("No TransferPreapproval contracts found for party.");
        return Ok(vec![]);
    }

    println!("\nFound {} TransferPreapproval contracts:", preapprovals.len());
    for preapproval in &preapprovals {
        let receiver = preapproval
            .pointer("/payload/receiver")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let provider = preapproval
            .pointer("/payload/provider")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let cid = preapproval
            .get("contractId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        println!("  - Receiver: {}", receiver);
        println!("    Provider: {}", provider);
        println!("    Contract ID: {}", cid);
    }
    println!();

    let mut cancelled = vec![];
    let url = format!("{}/commands/submit-and-wait", ledger_api_url);
    let num_preapprovals = preapprovals.len();

    for preapproval in &preapprovals {
        let preapproval_cid = preapproval
            .get("contractId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing contractId"))?;

        info!(cid = %preapproval_cid, "Cancelling preapproval...");

        let cancel_payload = json!({
            "commands": [{
                "ExerciseCommand": {
                    "templateId": "#splice-amulet:Splice.AmuletRules:TransferPreapproval",
                    "contractId": preapproval_cid,
                    "choice": "TransferPreapproval_Cancel",
                    "choiceArgument": {
                        "p": &party
                    }
                }
            }],
            "commandId": format!("cancel-preapproval-{}", Utc::now().timestamp_millis()),
            "actAs": [&party],
            "readAs": []
        });

        let response = client
            .post(&url)
            .bearer_auth(&jwt)
            .json(&cancel_payload)
            .send()
            .await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await?;

        if status.is_success() {
            let update_id = body
                .get("updateId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!("✓ Cancelled preapproval: {}", preapproval_cid);
            println!("  Update ID: {}", update_id);
            cancelled.push(update_id.to_string());
        } else {
            println!("✗ Failed to cancel preapproval: {}", preapproval_cid);
            println!("  Error: {:?}", body);
        }
    }

    println!(
        "\nCancelled {} of {} preapprovals.",
        cancelled.len(),
        num_preapprovals
    );
    Ok(cancelled)
}

/// Transfer Canton Coin using TransferPreapproval.
///
/// Uses interactive submission with the sender's Ed25519 signature.
pub async fn handle_transfer(
    sender_party_id: String,
    sender_private_key: String,
    receiver_party_id: String,
    amount: String,
    description: Option<String>,
) -> Result<SubmissionResult> {
    let ledger_api_url =
        std::env::var("LEDGER_API_URL").map_err(|_| anyhow!("LEDGER_API_URL not set"))?;
    let scan_api_url =
        std::env::var("SCAN_API_URL").map_err(|_| anyhow!("SCAN_API_URL not set"))?;
    let synchronizer_id =
        std::env::var("SYNCHRONIZER_ID").map_err(|_| anyhow!("SYNCHRONIZER_ID not set"))?;
    let jwt_provider =
        std::env::var("JWT_PROVIDER").map_err(|_| anyhow!("JWT_PROVIDER not set"))?;

    let provider = get_party_provider();
    let dso_party = get_dso_party(&scan_api_url).await?;

    // Parse Base58 private key
    let seed = parse_base58_private_key(&sender_private_key)?;

    info!(
        from = %sender_party_id,
        to = %receiver_party_id,
        amount = %amount,
        "Starting Canton Coin transfer via TransferPreapproval"
    );

    println!("\nTransferring Canton Coin...");
    println!("  From: {}", sender_party_id);
    println!("  To: {}", receiver_party_id);
    println!("  Amount: {} CC", amount);

    let client = create_client()?;

    // Fetch context contracts
    let amulet_rules = get_amulet_rules(&scan_api_url).await?;
    let open_mining_round = get_open_mining_round(&scan_api_url).await?;
    let external_party_amulet_rules = get_external_party_amulet_rules(&scan_api_url).await?;
    let featured_app_right = get_featured_app_right_for_provider(&scan_api_url, &provider).await?;

    // Find TransferPreapproval for receiver with our provider
    let preapproval = find_transfer_preapproval(
        &client,
        &ledger_api_url,
        &jwt_provider,
        &receiver_party_id,
        &provider,
    )
    .await?;

    // Fetch sender's amulet contracts
    let sender_amulets = find_amulets_for_party(
        &client,
        &ledger_api_url,
        &jwt_provider,
        &sender_party_id,
    )
    .await?;

    if sender_amulets.is_empty() {
        return Err(anyhow!(
            "No Amulet contracts found for sender {}",
            sender_party_id
        ));
    }

    let amulet_cids: Vec<&str> = sender_amulets.iter().map(|a| a.contract_id.as_str()).collect();

    // Build execution deadline
    let execute_before = (Utc::now() + chrono::Duration::seconds(30))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let reason = description.as_deref().unwrap_or("transfer-via-preapproval");

    // Build ExerciseCommand for TransferFactory_Transfer
    let commands = vec![json!({
        "ExerciseCommand": {
            "templateId": "#splice-api-token-transfer-instruction-v1:Splice.Api.Token.TransferInstructionV1:TransferFactory",
            "contractId": external_party_amulet_rules.contract_id,
            "choice": "TransferFactory_Transfer",
            "choiceArgument": {
                "expectedAdmin": dso_party,
                "transfer": {
                    "sender": sender_party_id,
                    "receiver": receiver_party_id,
                    "amount": amount,
                    "instrumentId": {
                        "admin": dso_party,
                        "id": "Amulet"
                    },
                    "inputHoldingCids": amulet_cids,
                    "requestedAt": Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
                    "executeBefore": execute_before,
                    "meta": {
                        "values": {
                            "splice.lfdecentralizedtrust.org/reason": reason
                        }
                    }
                },
                "extraArgs": {
                    "context": {
                        "values": {
                            "amulet-rules": {
                                "tag": "AV_ContractId",
                                "value": amulet_rules.contract_id
                            },
                            "open-round": {
                                "tag": "AV_ContractId",
                                "value": open_mining_round.contract_id
                            },
                            "featured-app-right": {
                                "tag": "AV_ContractId",
                                "value": featured_app_right.contract_id
                            },
                            "transfer-preapproval": {
                                "tag": "AV_ContractId",
                                "value": preapproval.contract_id
                            }
                        }
                    },
                    "meta": {
                        "values": {}
                    }
                }
            }
        }
    })];

    // Build disclosed contracts
    let disclosed_contracts = vec![
        json!({
            "contractId": amulet_rules.contract_id,
            "createdEventBlob": amulet_rules.created_event_blob,
            "synchronizerId": synchronizer_id,
            "templateId": amulet_rules.template_id
        }),
        json!({
            "contractId": open_mining_round.contract_id,
            "createdEventBlob": open_mining_round.created_event_blob,
            "synchronizerId": synchronizer_id,
            "templateId": open_mining_round.template_id
        }),
        json!({
            "contractId": external_party_amulet_rules.contract_id,
            "createdEventBlob": external_party_amulet_rules.created_event_blob,
            "synchronizerId": synchronizer_id,
            "templateId": external_party_amulet_rules.template_id
        }),
        json!({
            "contractId": featured_app_right.contract_id,
            "createdEventBlob": featured_app_right.created_event_blob,
            "synchronizerId": synchronizer_id,
            "templateId": featured_app_right.template_id
        }),
        json!({
            "contractId": preapproval.contract_id,
            "createdEventBlob": preapproval.created_event_blob,
            "synchronizerId": synchronizer_id,
            "templateId": preapproval.template_id
        }),
    ];

    // Submit via interactive submission
    let result = submit_interactive(
        &client,
        &ledger_api_url,
        &jwt_provider,
        &sender_party_id,
        &synchronizer_id,
        &seed,
        commands,
        disclosed_contracts,
    )
    .await?;

    println!("\n✓ Transfer successful!");
    println!("  Amount: {} CC", amount);
    println!("  From: {}", sender_party_id);
    println!("  To: {}", receiver_party_id);
    println!("  Update ID: {}", result.update_id);

    Ok(result)
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get ledger end offset
async fn get_ledger_end(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
) -> Result<String> {
    let url = format!("{}/state/ledger-end", api_url);
    let response: serde_json::Value = client
        .get(&url)
        .bearer_auth(jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    response
        .get("offset")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing offset in ledger-end response"))
}

/// Query active contracts for a party
async fn query_active_contracts(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    offset: &str,
) -> Result<Vec<serde_json::Value>> {
    let url = format!("{}/state/active-contracts", api_url);
    let payload = json!({
        "filter": {
            "filtersByParty": {
                party: {
                    "cumulative": []
                }
            }
        },
        "activeAtOffset": offset,
        "verbose": true
    });

    let response = client
        .post(&url)
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "Query active contracts failed (HTTP {}): {}",
            status,
            text
        ));
    }

    // Parse NDJSON response
    let mut contracts = vec![];
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(item) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(contract) = item.get("activeContract") {
                contracts.push(contract.clone());
            }
        }
    }

    Ok(contracts)
}

/// Find TransferPreapproval for a receiver with a specific provider
async fn find_transfer_preapproval(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    receiver: &str,
    required_provider: &str,
) -> Result<ContractInfo> {
    let offset = get_ledger_end(client, api_url, jwt).await?;
    let contracts = query_active_contracts(client, api_url, jwt, receiver, &offset).await?;

    for contract in contracts {
        let template_id = contract
            .get("templateId")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if template_id.contains("TransferPreapproval") && !template_id.contains("Proposal") {
            let provider = contract
                .pointer("/payload/provider")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if provider == required_provider {
                let contract_id = contract
                    .get("contractId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing contractId"))?
                    .to_string();

                let created_event_blob = contract
                    .get("createdEventBlob")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing createdEventBlob for TransferPreapproval"))?
                    .to_string();

                return Ok(ContractInfo {
                    contract_id,
                    template_id: template_id.to_string(),
                    created_event_blob,
                });
            }
        }
    }

    Err(anyhow!(
        "No TransferPreapproval found for receiver {} with provider {}. Run 'preapproval request' and 'preapproval accept' first.",
        receiver,
        required_provider
    ))
}

/// Find Amulet contracts for a party
async fn find_amulets_for_party(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
) -> Result<Vec<ContractInfo>> {
    let offset = get_ledger_end(client, api_url, jwt).await?;
    let contracts = query_active_contracts(client, api_url, jwt, party, &offset).await?;

    let mut amulets = vec![];
    for contract in contracts {
        let template_id = contract
            .get("templateId")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if template_id.contains("Splice.Amulet:Amulet") {
            let contract_id = contract
                .get("contractId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing contractId"))?
                .to_string();

            let created_event_blob = contract
                .get("createdEventBlob")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            amulets.push(ContractInfo {
                contract_id,
                template_id: template_id.to_string(),
                created_event_blob,
            });
        }
    }

    Ok(amulets)
}

//! AdvancedPaymentRequest command implementations for devnet.
//!
//! Uses interactive submission with Ed25519 signing for external parties.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::context::{create_client, ContractBlobsContext};
use crate::interactive::submit_interactive;
use crate::signing::parse_base58_private_key;

/// Create a new AdvancedPaymentRequest via AppService choice (app action)
/// This requires an existing AppService contract between app and provider
pub async fn handle_create_request(
    service_cid: String,
    amount: String,
    minimum: String,
    expires: String,
    description: Option<String>,
    reference: Option<String>,
    user: String,
) -> Result<()> {
    info!("Creating AdvancedPaymentRequest via AppService (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let app_private_key = std::env::var("PARTY_APP_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PRIVATE_KEY not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse app's private key (Base58 format)
    let app_seed = parse_base58_private_key(&app_private_key)?;

    let client = create_client()?;
    let template_id = format!("{}:AppService:AppService", package_id);

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Build command - exercise AppService_CreatePaymentRequest choice
    let commands = vec![json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": service_cid,
            "choice": "AppService_CreatePaymentRequest",
            "choiceArgument": {
                "owner": user,
                "lockedAmount": amount,
                "minimumAmount": minimum,
                "expiresAt": expires,
                "description": description,
                "reference": reference
            }
        }
    })];

    debug!("Create request command: {}", serde_json::to_string_pretty(&commands)?);

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_app,
        &synchronizer_id,
        &app_seed,
        commands,
        vec![], // No disclosed contracts needed
    )
    .await?;

    info!(
        submission_id = %result.submission_id,
        update_id = %result.update_id,
        "AdvancedPaymentRequest created via AppService"
    );

    // Fetch update to get contract ID
    let update_payload = json!({
        "actAs": [party_app],
        "updateId": result.update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app: {
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
        .post(&format!("{}/updates/update-by-id", api_url))
        .bearer_auth(&jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract contract ID
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AdvancedPaymentRequest:AdvancedPaymentRequest") {
                        let cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("AdvancedPaymentRequest created successfully!");
                        println!("Contract ID: {}", cid);
                        println!("Amount: {} CC", amount);
                        println!("Minimum: {} CC", minimum);
                        println!("Expires: {}", expires);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("AdvancedPaymentRequest created");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Accept an AdvancedPaymentRequest (owner action)
pub async fn handle_accept_request(
    request_cid: String,
    amulet_cids: Vec<String>,
    party_id: String,
    private_key: String,
) -> Result<()> {
    info!(request_cid = %request_cid, "Accepting AdvancedPaymentRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse user's private key (Base58 format)
    let user_seed = parse_base58_private_key(&private_key)?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client()?;
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Build command (beneficiaries built in DAML from provider field)
    let commands = vec![json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": request_cid,
            "choice": "AdvancedPaymentRequest_Accept",
            "choiceArgument": {
                "ownerInputs": amulet_cids,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    debug!("Accept command: {}", serde_json::to_string_pretty(&commands)?);

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_id,
        &synchronizer_id,
        &user_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await?;

    info!(
        submission_id = %result.submission_id,
        update_id = %result.update_id,
        "AdvancedPaymentRequest accepted"
    );

    // Fetch update to get new AdvancedPayment contract ID
    let update_payload = json!({
        "actAs": [&party_id],
        "updateId": result.update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_id: {
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
        .post(&format!("{}/updates/update-by-id", api_url))
        .bearer_auth(&jwt)
        .json(&update_payload)
        .send()
        .await?;

    let update_text = update_response.text().await?;
    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract AdvancedPayment contract ID
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AdvancedPayment:AdvancedPayment") {
                        let cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let locked_amount = created
                            .pointer("/createArgument/lockedAmount")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("AdvancedPayment created successfully!");
                        println!("Contract ID: {}", cid);
                        println!("Locked Amount: {} CC", locked_amount);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("AdvancedPaymentRequest accepted");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Reject an AdvancedPaymentRequest (owner action)
pub async fn handle_reject_request(
    request_cid: String,
    reason: Option<String>,
    party_id: String,
    private_key: String,
) -> Result<()> {
    info!(request_cid = %request_cid, "Rejecting AdvancedPaymentRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse user's private key (Base58 format)
    let user_seed = parse_base58_private_key(&private_key)?;

    let client = create_client()?;
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Build command
    let commands = vec![json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": request_cid,
            "choice": "AdvancedPaymentRequest_Reject",
            "choiceArgument": {
                "reason": reason
            }
        }
    })];

    debug!(
        "Reject command: {}",
        serde_json::to_string_pretty(&commands)?
    );

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_id,
        &synchronizer_id,
        &user_seed,
        commands,
        vec![],
    )
    .await?;

    println!("AdvancedPaymentRequest rejected successfully!");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Cancel an AdvancedPaymentRequest (app action)
pub async fn handle_cancel_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Canceling AdvancedPaymentRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let app_private_key = std::env::var("PARTY_APP_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PRIVATE_KEY not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse app's private key (Base58 format)
    let app_seed = parse_base58_private_key(&app_private_key)?;

    let client = create_client()?;
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Build command
    let commands = vec![json!({
        "ExerciseCommand": {
            "templateId": template_id,
            "contractId": request_cid,
            "choice": "AdvancedPaymentRequest_Cancel",
            "choiceArgument": {}
        }
    })];

    debug!(
        "Cancel command: {}",
        serde_json::to_string_pretty(&commands)?
    );

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_app,
        &synchronizer_id,
        &app_seed,
        commands,
        vec![],
    )
    .await?;

    println!("AdvancedPaymentRequest canceled successfully!");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

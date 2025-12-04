//! AdvancedPayment command implementations for devnet.
//!
//! Uses interactive submission with Ed25519 signing for external parties.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::context::{create_client, ContractBlobsContext};
use crate::interactive::submit_interactive;
use crate::signing::parse_base58_private_key;

/// Seller withdraws amount from AdvancedPayment
pub async fn handle_withdraw(payment_cid: String, amount: String, reason: Option<String>) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Withdrawing from AdvancedPayment (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_seller = std::env::var("PARTY_SELLER")
        .map_err(|_| anyhow::anyhow!("PARTY_SELLER not set"))?;
    let seller_private_key = std::env::var("PARTY_SELLER_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("PARTY_SELLER_PRIVATE_KEY not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse app's private key (Base58 format)
    let seller_seed = parse_base58_private_key(&seller_private_key)?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client()?;
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

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
            "contractId": payment_cid,
            "choice": "AdvancedPayment_Withdraw",
            "choiceArgument": {
                "amount": amount,
                "appTransferContext": context.build_app_transfer_context(),
                "withdrawReason": reason
            }
        }
    })];

    debug!("Withdraw command: {}", serde_json::to_string_pretty(&commands)?);

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_seller,
        &synchronizer_id,
        &seller_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await?;

    info!(
        submission_id = %result.submission_id,
        update_id = %result.update_id,
        "Withdrawal successful"
    );

    // Fetch update to see result
    let update_payload = json!({
        "actAs": [party_seller],
        "updateId": result.update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_seller: {
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

    println!("Withdrawal successful!");
    println!("Amount withdrawn: {} CC", amount);

    // Check if new AdvancedPayment was created (remaining funds)
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AdvancedPayment:AdvancedPayment") {
                        let new_cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let remaining = created
                            .pointer("/createArgument/lockedAmount")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("New AdvancedPayment contract: {}", new_cid);
                        println!("Remaining locked: {} CC", remaining);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("Contract fully withdrawn (no remaining funds)");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Buyer unlocks partial amount from AdvancedPayment
pub async fn handle_unlock(
    payment_cid: String,
    amount: String,
    party_id: String,
    private_key: String,
) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Unlocking from AdvancedPayment (devnet)");

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
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

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
            "contractId": payment_cid,
            "choice": "AdvancedPayment_Unlock",
            "choiceArgument": {
                "amount": amount,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    debug!("Unlock command: {}", serde_json::to_string_pretty(&commands)?);

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
        "Unlock successful"
    );

    // Fetch update to see new contract
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

    println!("Unlock successful!");
    println!("Amount unlocked: {} CC", amount);

    // Find new AdvancedPayment contract
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AdvancedPayment:AdvancedPayment") {
                        let new_cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let remaining = created
                            .pointer("/createArgument/lockedAmount")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("New AdvancedPayment contract: {}", new_cid);
                        println!("Remaining locked: {} CC", remaining);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Seller cancels AdvancedPayment and returns funds to buyer
pub async fn handle_cancel(payment_cid: String) -> Result<()> {
    info!(payment_cid = %payment_cid, "Canceling AdvancedPayment (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_seller = std::env::var("PARTY_SELLER")
        .map_err(|_| anyhow::anyhow!("PARTY_SELLER not set"))?;
    let seller_private_key = std::env::var("PARTY_SELLER_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("PARTY_SELLER_PRIVATE_KEY not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse app's private key (Base58 format)
    let seller_seed = parse_base58_private_key(&seller_private_key)?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client()?;
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

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
            "contractId": payment_cid,
            "choice": "AdvancedPayment_Cancel",
            "choiceArgument": {
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    debug!("Cancel command: {}", serde_json::to_string_pretty(&commands)?);

    // Submit via interactive submission with Ed25519 signing
    let result = submit_interactive(
        &client,
        &api_url,
        &jwt,
        &party_seller,
        &synchronizer_id,
        &seller_seed,
        commands,
        context.build_disclosed_contracts(),
    )
    .await?;

    println!("AdvancedPayment canceled successfully!");
    println!("Funds returned to buyer");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Buyer expires AdvancedPayment after lock expiry
pub async fn handle_expire(
    payment_cid: String,
    party_id: String,
    private_key: String,
) -> Result<()> {
    info!(payment_cid = %payment_cid, "Expiring AdvancedPayment (devnet)");

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
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

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
            "contractId": payment_cid,
            "choice": "AdvancedPayment_Expire",
            "choiceArgument": {
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    debug!("Expire command: {}", serde_json::to_string_pretty(&commands)?);

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

    println!("AdvancedPayment expired successfully!");
    println!("All funds returned to buyer");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Buyer tops up AdvancedPayment with additional funds and extends expiry
pub async fn handle_topup(
    payment_cid: String,
    amount: String,
    new_expires: String,
    amulet_cids: Vec<String>,
    party_id: String,
    private_key: String,
) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Topping up AdvancedPayment (devnet)");

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
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

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
            "contractId": payment_cid,
            "choice": "AdvancedPayment_TopUp",
            "choiceArgument": {
                "topUpInputs": amulet_cids,
                "topUpAmount": amount,
                "newExpiresAt": new_expires,
                "appTransferContext": context.build_app_transfer_context()
            }
        }
    })];

    debug!("TopUp command: {}", serde_json::to_string_pretty(&commands)?);

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
        "TopUp successful"
    );

    // Fetch update to see new contract
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

    println!("TopUp successful!");
    println!("Amount added: {} CC", amount);
    println!("New expiry: {}", new_expires);

    // Find new AdvancedPayment contract
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AdvancedPayment:AdvancedPayment") {
                        let new_cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let total_locked = created
                            .pointer("/createArgument/lockedAmount")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("New AdvancedPayment contract: {}", new_cid);
                        println!("Total locked: {} CC", total_locked);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

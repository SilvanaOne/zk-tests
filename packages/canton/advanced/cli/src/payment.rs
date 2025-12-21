//! AdvancedPayment command implementations.

use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::context::ContractBlobsContext;
use crate::url::create_client_with_localhost_resolution;

/// Traffic cost estimation from /v2/interactive-submission/prepare
#[derive(Debug, Clone, Default)]
pub struct TrafficInfo {
    /// Request traffic cost in bytes
    pub request: Option<u64>,
    /// Response traffic cost in bytes
    pub response: Option<u64>,
    /// Total traffic cost in bytes
    pub total: Option<u64>,
}

/// Extract userId (sub claim) from JWT token.
fn extract_user_id_from_jwt(jwt: &str) -> Result<String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid JWT format: expected 3 parts"));
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| anyhow::anyhow!("Failed to decode JWT payload: {}", e))?;

    let claims: Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| anyhow::anyhow!("Failed to parse JWT claims: {}", e))?;

    claims
        .get("sub")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Missing 'sub' claim in JWT"))
}

/// Get traffic cost estimation for a command without executing it.
/// Uses /v2/interactive-submission/prepare endpoint.
/// Returns error if traffic estimation is not available - transaction will not proceed.
async fn get_traffic_estimation(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
    synchronizer_id: &str,
    commands: Vec<Value>,
    disclosed_contracts: Vec<Value>,
) -> Result<TrafficInfo> {
    let user_id = extract_user_id_from_jwt(jwt)?;

    let command_id = format!("traffic-estimate-{}", chrono::Utc::now().timestamp_millis());

    let payload = json!({
        "userId": user_id,
        "commandId": command_id,
        "actAs": [party],
        "readAs": [party],
        "synchronizerId": synchronizer_id,
        "packageIdSelectionPreference": [],
        "verboseHashing": false,
        "commands": commands,
        "disclosedContracts": disclosed_contracts
    });

    let url = format!("{}v2/interactive-submission/prepare", api_url);
    debug!("Calling prepare endpoint for traffic estimation: {}", url);

    let response = client
        .post(&url)
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to call prepare endpoint: {}", e))?;

    let status = response.status();
    let text = response.text().await
        .map_err(|e| anyhow::anyhow!("Failed to read prepare response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Prepare endpoint returned error ({}): {}",
            status,
            text
        ));
    }

    let body: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Failed to parse prepare response: {}", e))?;

    let cost = body
        .get("costEstimation")
        .ok_or_else(|| anyhow::anyhow!("Missing costEstimation in prepare response"))?;

    Ok(TrafficInfo {
        request: cost
            .get("confirmationRequestTrafficCostEstimation")
            .and_then(|v| v.as_u64()),
        response: cost
            .get("confirmationResponseTrafficCostEstimation")
            .and_then(|v| v.as_u64()),
        total: cost
            .get("totalTrafficCostEstimation")
            .and_then(|v| v.as_u64()),
    })
}

/// Print traffic info if available
fn print_traffic_info(traffic: &TrafficInfo) {
    println!();
    println!("Traffic Cost Estimation:");
    if let Some(req) = traffic.request {
        println!("  Request:  {} bytes", req);
    }
    if let Some(resp) = traffic.response {
        println!("  Response: {} bytes", resp);
    }
    if let Some(total) = traffic.total {
        println!("  Total:    {} bytes", total);
    }
}

/// Seller withdraws amount from AdvancedPayment
pub async fn handle_withdraw(payment_cid: String, amount: String, reason: Option<String>) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Withdrawing from AdvancedPayment");

    let party_seller =
        std::env::var("PARTY_SELLER").map_err(|_| anyhow::anyhow!("PARTY_SELLER not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("withdraw-advanced-payment-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    // Build command for both traffic estimation and actual execution
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
    let disclosed_contracts = context.build_disclosed_contracts();

    // Get traffic estimation first (required - will fail if not available)
    info!("Getting traffic cost estimation...");
    let traffic = get_traffic_estimation(
        &client,
        &provider_api_url,
        &provider_jwt,
        &party_seller,
        &synchronizer_id,
        commands.clone(),
        disclosed_contracts.clone(),
    )
    .await?;

    // Print traffic info immediately
    print_traffic_info(&traffic);

    let payload = json!({
        "commands": commands,
        "disclosedContracts": disclosed_contracts,
        "commandId": cmdid,
        "actAs": [party_seller],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Withdraw payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", provider_api_url))
        .bearer_auth(&provider_jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to withdraw from AdvancedPayment: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "Withdrawal successful");

    // Fetch update to see result
    let update_payload = json!({
        "actAs": [party_seller],
        "updateId": update_id,
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
        .post(&format!("{}v2/updates/update-by-id", provider_api_url))
        .bearer_auth(&provider_jwt)
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
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("Contract fully withdrawn (no remaining funds)");
    Ok(())
}

/// Buyer unlocks partial amount from AdvancedPayment
pub async fn handle_unlock(payment_cid: String, amount: String) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Unlocking from AdvancedPayment");

    let party_buyer = std::env::var("PARTY_BUYER")
        .map_err(|_| anyhow::anyhow!("PARTY_BUYER not set"))?;
    let user_api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set"))?;
    let user_jwt =
        std::env::var("APP_USER_JWT").map_err(|_| anyhow::anyhow!("APP_USER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("unlock-advanced-payment-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": payment_cid,
                "choice": "AdvancedPayment_Unlock",
                "choiceArgument": {
                    "amount": amount,
                    "appTransferContext": context.build_app_transfer_context()
                }
            }
        }],
        "disclosedContracts": context.build_disclosed_contracts(),
        "commandId": cmdid,
        "actAs": [party_buyer],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Unlock payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", user_api_url))
        .bearer_auth(&user_jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to unlock from AdvancedPayment: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "Unlock successful");

    // Fetch update to see new contract
    let update_payload = json!({
        "actAs": [party_buyer],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_buyer: {
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
        .post(&format!("{}v2/updates/update-by-id", user_api_url))
        .bearer_auth(&user_jwt)
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
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Seller cancels AdvancedPayment and returns funds to buyer
pub async fn handle_cancel(payment_cid: String) -> Result<()> {
    info!(payment_cid = %payment_cid, "Canceling AdvancedPayment");

    let party_seller =
        std::env::var("PARTY_SELLER").map_err(|_| anyhow::anyhow!("PARTY_SELLER not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("cancel-advanced-payment-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": payment_cid,
                "choice": "AdvancedPayment_Cancel",
                "choiceArgument": {
                    "appTransferContext": context.build_app_transfer_context()
                }
            }
        }],
        "disclosedContracts": context.build_disclosed_contracts(),
        "commandId": cmdid,
        "actAs": [party_seller],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Cancel payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", provider_api_url))
        .bearer_auth(&provider_jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to cancel AdvancedPayment: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AdvancedPayment canceled successfully!");
    println!("Funds returned to buyer");
    println!("Update ID: {}", update_id);
    Ok(())
}

/// Buyer expires AdvancedPayment after lock expiry
pub async fn handle_expire(payment_cid: String) -> Result<()> {
    info!(payment_cid = %payment_cid, "Expiring AdvancedPayment");

    let party_buyer = std::env::var("PARTY_BUYER")
        .map_err(|_| anyhow::anyhow!("PARTY_BUYER not set"))?;
    let user_api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set"))?;
    let user_jwt =
        std::env::var("APP_USER_JWT").map_err(|_| anyhow::anyhow!("APP_USER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("expire-advanced-payment-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": payment_cid,
                "choice": "AdvancedPayment_Expire",
                "choiceArgument": {
                    "appTransferContext": context.build_app_transfer_context()
                }
            }
        }],
        "disclosedContracts": context.build_disclosed_contracts(),
        "commandId": cmdid,
        "actAs": [party_buyer],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Expire payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", user_api_url))
        .bearer_auth(&user_jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to expire AdvancedPayment: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AdvancedPayment expired successfully!");
    println!("All funds returned to buyer");
    println!("Update ID: {}", update_id);
    Ok(())
}

/// Buyer tops up AdvancedPayment with additional funds and extends expiry
pub async fn handle_topup(
    payment_cid: String,
    amount: String,
    new_expires: String,
    amulet_cids: Vec<String>,
) -> Result<()> {
    info!(payment_cid = %payment_cid, amount = %amount, "Topping up AdvancedPayment");

    let party_buyer = std::env::var("PARTY_BUYER")
        .map_err(|_| anyhow::anyhow!("PARTY_BUYER not set"))?;
    let user_api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set"))?;
    let user_jwt =
        std::env::var("APP_USER_JWT").map_err(|_| anyhow::anyhow!("APP_USER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Fetch contract blobs context for AppTransferContext
    info!("Fetching contract context from Scan API...");
    let context = ContractBlobsContext::fetch().await?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("topup-advanced-payment-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AdvancedPayment:AdvancedPayment", package_id);

    let payload = json!({
        "commands": [{
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
        }],
        "disclosedContracts": context.build_disclosed_contracts(),
        "commandId": cmdid,
        "actAs": [party_buyer],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("TopUp payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", user_api_url))
        .bearer_auth(&user_jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to top up AdvancedPayment: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "TopUp successful");

    // Fetch update to see new contract
    let update_payload = json!({
        "actAs": [party_buyer],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_buyer: {
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
        .post(&format!("{}v2/updates/update-by-id", user_api_url))
        .bearer_auth(&user_jwt)
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
                        return Ok(());
                    }
                }
            }
        }
    }

    Ok(())
}

//! AdvancedPaymentRequest command implementations.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::context::ContractBlobsContext;
use crate::url::create_client_with_localhost_resolution;

/// Create a new AdvancedPaymentRequest (provider action)
pub async fn handle_create_request(
    amount: String,
    minimum: String,
    expires: String,
) -> Result<()> {
    info!("Creating AdvancedPaymentRequest");

    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set"))?;
    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PROVIDER not set"))?;
    let party_dso =
        std::env::var("PARTY_DSO").map_err(|_| anyhow::anyhow!("PARTY_DSO not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!(
        "create-advanced-payment-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    let payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "dso": party_dso,
                    "owner": party_app_user,
                    "provider": party_app_provider,
                    "lockedAmount": amount,
                    "minimumAmount": minimum,
                    "expiresAt": expires
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_provider],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Create payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to create AdvancedPaymentRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "AdvancedPaymentRequest created");

    // Fetch update to get contract ID
    let update_payload = json!({
        "actAs": [party_app_provider],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app_provider: {
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
        .post(&format!("{}v2/updates/update-by-id", provider_api_url))
        .bearer_auth(&provider_jwt)
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
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("AdvancedPaymentRequest created (update_id: {})", update_id);
    Ok(())
}

/// Accept an AdvancedPaymentRequest (owner action)
pub async fn handle_accept_request(request_cid: String, amulet_cids: Vec<String>) -> Result<()> {
    info!(request_cid = %request_cid, "Accepting AdvancedPaymentRequest");

    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set"))?;
    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PROVIDER not set"))?;
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
    let cmdid = format!(
        "accept-advanced-payment-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    // Beneficiaries: Optional [AppRewardBeneficiary] - array for Some, null for None
    let beneficiaries = json!([{
        "beneficiary": party_app_provider,
        "weight": "1.0"
    }]);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AdvancedPaymentRequest_Accept",
                "choiceArgument": {
                    "ownerInputs": amulet_cids,
                    "appTransferContext": context.build_app_transfer_context(),
                    "beneficiaries": beneficiaries
                }
            }
        }],
        "disclosedContracts": context.build_disclosed_contracts(),
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!("Accept payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to accept AdvancedPaymentRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "AdvancedPaymentRequest accepted");

    // Fetch update to get new AdvancedPayment contract ID
    let update_payload = json!({
        "actAs": [party_app_user],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_app_user: {
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
                        return Ok(());
                    }
                }
            }
        }
    }

    println!(
        "AdvancedPaymentRequest accepted (update_id: {})",
        update_id
    );
    Ok(())
}

/// Decline an AdvancedPaymentRequest (owner action)
pub async fn handle_decline_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Declining AdvancedPaymentRequest");

    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set"))?;
    let user_api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set"))?;
    let user_jwt =
        std::env::var("APP_USER_JWT").map_err(|_| anyhow::anyhow!("APP_USER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!(
        "decline-advanced-payment-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AdvancedPaymentRequest_Decline",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Decline payload: {}",
        serde_json::to_string_pretty(&payload)?
    );

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
            "Failed to decline AdvancedPaymentRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AdvancedPaymentRequest declined successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

/// Cancel an AdvancedPaymentRequest (provider action)
pub async fn handle_cancel_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Canceling AdvancedPaymentRequest");

    let party_app_provider = std::env::var("PARTY_APP_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PROVIDER not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!(
        "cancel-advanced-payment-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!(
        "{}:AdvancedPaymentRequest:AdvancedPaymentRequest",
        package_id
    );

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AdvancedPaymentRequest_Cancel",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_provider],
        "readAs": [],
        "workflowId": "AdvancedPayment",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Cancel payload: {}",
        serde_json::to_string_pretty(&payload)?
    );

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
            "Failed to cancel AdvancedPaymentRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AdvancedPaymentRequest canceled successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

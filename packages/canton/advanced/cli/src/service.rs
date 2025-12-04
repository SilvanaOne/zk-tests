//! AppService and AppServiceRequest command implementations.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::url::create_client_with_localhost_resolution;

/// Create a new AppServiceRequest (app action)
/// App creates a request to establish service relationship with provider
pub async fn handle_create_service_request(service_description: Option<String>) -> Result<()> {
    info!("Creating AppServiceRequest");

    let party_app =
        std::env::var("PARTY_APP").map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
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
        "create-app-service-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    let payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "dso": party_dso,
                    "app": party_app,
                    "provider": party_provider,
                    "serviceDescription": service_description
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app],
        "readAs": [],
        "workflowId": "AppService",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Create payload: {}",
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
            "Failed to create AppServiceRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "AppServiceRequest created");

    // Fetch update to get contract ID
    let update_payload = json!({
        "actAs": [party_app],
        "updateId": update_id,
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
                    if template.contains(":AppServiceRequest:AppServiceRequest") {
                        let cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("AppServiceRequest created successfully!");
                        println!("Contract ID: {}", cid);
                        println!("App: {}", party_app);
                        println!("Provider: {}", party_provider);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("AppServiceRequest created (update_id: {})", update_id);
    Ok(())
}

/// List pending AppServiceRequest contracts
pub async fn handle_list_service_requests() -> Result<()> {
    info!("Listing AppServiceRequest contracts");

    let party_app =
        std::env::var("PARTY_APP").map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;

    // Get ledger end offset
    let ledger_end_url = format!("{}v2/state/ledger-end", provider_api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(&provider_jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

    let query = json!({
        "activeAtOffset": offset,
        "filter": {
            "filtersByParty": {
                &party_app: {}
            }
        },
        "verbose": true
    });

    let contracts_url = format!("{}v2/state/active-contracts?limit=500", provider_api_url);
    let contracts: Vec<serde_json::Value> = client
        .post(&contracts_url)
        .bearer_auth(&provider_jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let template_pattern = format!("{}:AppServiceRequest:AppServiceRequest", package_id);
    let mut found = false;

    println!("\n=== Pending AppServiceRequest Contracts ===\n");

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                        if template_id.contains(&template_pattern) {
                            found = true;
                            let cid = created
                                .get("contractId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let app = created
                                .pointer("/createArgument/app")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let provider = created
                                .pointer("/createArgument/provider")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");

                            println!("Contract ID: {}", cid);
                            println!("  App: {}", app);
                            println!("  Provider: {}", provider);
                            println!();
                        }
                    }
                }
            }
        }
    }

    if !found {
        println!("  (none)");
    }

    Ok(())
}

/// Accept an AppServiceRequest (provider action)
/// Provider accepts, creating an AppService contract
pub async fn handle_accept_service_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Accepting AppServiceRequest");

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
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
        "accept-app-service-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AppServiceRequest_Accept",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [party_provider],
        "readAs": [],
        "workflowId": "AppService",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Accept payload: {}",
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
            "Failed to accept AppServiceRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!(update_id = %update_id, "AppServiceRequest accepted");

    // Fetch update to get new AppService contract ID
    let update_payload = json!({
        "actAs": [party_provider],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        &party_provider: {
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

    // Extract AppService contract ID
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":AppService:AppService") {
                        let cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("AppService created successfully!");
                        println!("Contract ID: {}", cid);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!(
        "AppServiceRequest accepted (update_id: {})",
        update_id
    );
    Ok(())
}

/// Reject an AppServiceRequest (provider action)
pub async fn handle_reject_service_request(request_cid: String, reason: Option<String>) -> Result<()> {
    info!(request_cid = %request_cid, "Rejecting AppServiceRequest");

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
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
        "reject-app-service-request-{}",
        chrono::Utc::now().timestamp()
    );
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AppServiceRequest_Reject",
                "choiceArgument": {
                    "reason": reason
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_provider],
        "readAs": [],
        "workflowId": "AppService",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Reject payload: {}",
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
            "Failed to reject AppServiceRequest: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AppServiceRequest rejected successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

/// List active AppService contracts
pub async fn handle_list_services() -> Result<()> {
    info!("Listing AppService contracts");

    let party_app =
        std::env::var("PARTY_APP").map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;

    // Get ledger end offset
    let ledger_end_url = format!("{}v2/state/ledger-end", provider_api_url);
    let ledger_end: serde_json::Value = client
        .get(&ledger_end_url)
        .bearer_auth(&provider_jwt)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let offset = ledger_end["offset"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Unable to get ledger end offset"))?;

    let query = json!({
        "activeAtOffset": offset,
        "filter": {
            "filtersByParty": {
                &party_app: {}
            }
        },
        "verbose": true
    });

    let contracts_url = format!("{}v2/state/active-contracts?limit=500", provider_api_url);
    let contracts: Vec<serde_json::Value> = client
        .post(&contracts_url)
        .bearer_auth(&provider_jwt)
        .json(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let template_pattern = format!("{}:AppService:AppService", package_id);
    let mut found = false;

    println!("\n=== Active AppService Contracts ===\n");

    for contract in contracts {
        if let Some(entry) = contract.get("contractEntry") {
            if let Some(js_contract) = entry.get("JsActiveContract") {
                if let Some(created) = js_contract.get("createdEvent") {
                    if let Some(template_id) = created.get("templateId").and_then(|v| v.as_str()) {
                        if template_id.contains(&template_pattern) {
                            found = true;
                            let cid = created
                                .get("contractId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let app = created
                                .pointer("/createArgument/app")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let provider = created
                                .pointer("/createArgument/provider")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");

                            println!("Contract ID: {}", cid);
                            println!("  App: {}", app);
                            println!("  Provider: {}", provider);
                            println!();
                        }
                    }
                }
            }
        }
    }

    if !found {
        println!("  (none)");
    }

    Ok(())
}

/// Terminate an AppService (provider action)
pub async fn handle_terminate_service(service_cid: String) -> Result<()> {
    info!(service_cid = %service_cid, "Terminating AppService");

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_API_URL not set"))?;
    let provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_PROVIDER_JWT not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    let client = create_client_with_localhost_resolution()?;
    let cmdid = format!("terminate-app-service-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:AppService:AppService", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": service_cid,
                "choice": "AppService_Terminate",
                "choiceArgument": {}
            }
        }],
        "commandId": cmdid,
        "actAs": [party_provider],
        "readAs": [],
        "workflowId": "AppService",
        "synchronizerId": synchronizer_id
    });

    debug!(
        "Terminate payload: {}",
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
            "Failed to terminate AppService: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_resp: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json_resp
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AppService terminated successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

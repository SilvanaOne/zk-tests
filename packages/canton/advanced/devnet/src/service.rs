//! AppService and AppServiceRequest command implementations for devnet.
//!
//! Uses interactive submission with Ed25519 signing for external parties.
//! Provider (orderbook-operator-1) is an internal party and uses standard submission.

use anyhow::Result;
use serde_json::json;
use tracing::{debug, info};

use crate::context::{create_client, ContractBlobsContext};
use crate::interactive::submit_interactive;
use crate::signing::{extract_user_id_from_jwt, parse_base58_private_key};

/// Create a new AppServiceRequest (app action)
/// App requests a service relationship with provider
pub async fn handle_create_service_request(service_description: Option<String>) -> Result<()> {
    info!("Creating AppServiceRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let app_private_key = std::env::var("PARTY_APP_PRIVATE_KEY")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_PRIVATE_KEY not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;
    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set"))?;

    // Parse app's private key (Base58 format)
    let app_seed = parse_base58_private_key(&app_private_key)?;

    // Fetch DSO party from scan API
    let context = ContractBlobsContext::fetch().await?;
    let party_dso = context.dso_party;

    let client = create_client()?;
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Build command
    let commands = vec![json!({
        "CreateCommand": {
            "templateId": template_id,
            "createArguments": {
                "dso": party_dso,
                "app": party_app,
                "provider": party_provider,
                "serviceDescription": service_description
            }
        }
    })];

    debug!(
        "Create service request command: {}",
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
        vec![], // No disclosed contracts needed for create
    )
    .await?;

    info!(
        submission_id = %result.submission_id,
        update_id = %result.update_id,
        "AppServiceRequest created"
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
                    if template.contains(":AppServiceRequest:AppServiceRequest") {
                        let cid = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("AppServiceRequest created successfully!");
                        println!("Contract ID: {}", cid);
                        println!("App: {}", party_app);
                        println!("Provider: {}", party_provider);
                        println!("Submission ID: {}", result.submission_id);
                        println!("Update ID: {}", result.update_id);
                        return Ok(());
                    }
                }
            }
        }
    }

    println!("AppServiceRequest created");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// Accept an AppServiceRequest (provider action)
/// Provider is an internal party, so we use standard submission (not interactive)
pub async fn handle_accept_service_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Accepting AppServiceRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let user_id = extract_user_id_from_jwt(&jwt)?;

    let client = create_client()?;
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    // Provider is an internal party - use standard submission (not interactive)
    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": request_cid,
                "choice": "AppServiceRequest_Accept",
                "choiceArgument": {}
            }
        }],
        "userId": user_id,
        "commandId": command_id,
        "actAs": [party_provider],
        "readAs": [party_provider]
    });

    debug!(
        "Accept service request payload: {}",
        serde_json::to_string_pretty(&payload)?
    );

    let response = client
        .post(&format!("{}commands/submit-and-wait", api_url))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Accept service request failed (HTTP {}): {}",
            status,
            text
        ));
    }

    info!(response = %text, "AppServiceRequest accepted");

    // Parse response to extract AppService contract ID
    let response_json: serde_json::Value = serde_json::from_str(&text)?;

    if let Some(update_id) = response_json.get("updateId").and_then(|v| v.as_str()) {
        // Fetch update to get AppService contract ID
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
            .post(&format!("{}/updates/update-by-id", api_url))
            .bearer_auth(&jwt)
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
                    if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str())
                    {
                        if template.contains(":AppService:AppService") {
                            let cid = created
                                .pointer("/contractId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            println!("AppService created successfully!");
                            println!("Contract ID: {}", cid);
                            println!("Update ID: {}", update_id);
                            return Ok(());
                        }
                    }
                }
            }
        }

        println!("AppServiceRequest accepted");
        println!("Update ID: {}", update_id);
    } else {
        println!("AppServiceRequest accepted");
        println!("Response: {}", text);
    }

    Ok(())
}

/// Reject an AppServiceRequest (provider action)
/// Provider is an internal party, so we use standard submission (not interactive)
pub async fn handle_reject_service_request(request_cid: String, reason: Option<String>) -> Result<()> {
    info!(request_cid = %request_cid, "Rejecting AppServiceRequest (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let user_id = extract_user_id_from_jwt(&jwt)?;

    let client = create_client()?;
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    // Provider is an internal party - use standard submission (not interactive)
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
        "userId": user_id,
        "commandId": command_id,
        "actAs": [party_provider],
        "readAs": [party_provider]
    });

    debug!(
        "Reject service request payload: {}",
        serde_json::to_string_pretty(&payload)?
    );

    let response = client
        .post(&format!("{}commands/submit-and-wait", api_url))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Reject service request failed (HTTP {}): {}",
            status,
            text
        ));
    }

    info!(response = %text, "AppServiceRequest rejected");

    let response_json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AppServiceRequest rejected successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

/// Cancel an AppServiceRequest (app action)
pub async fn handle_cancel_service_request(request_cid: String) -> Result<()> {
    info!(request_cid = %request_cid, "Canceling AppServiceRequest (devnet)");

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
    let template_id = format!("{}:AppServiceRequest:AppServiceRequest", package_id);

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
            "choice": "AppServiceRequest_Cancel",
            "choiceArgument": {}
        }
    })];

    debug!(
        "Cancel service request command: {}",
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

    println!("AppServiceRequest canceled successfully!");
    println!("Submission ID: {}", result.submission_id);
    println!("Update ID: {}", result.update_id);
    Ok(())
}

/// List active AppService contracts
pub async fn handle_list_services() -> Result<()> {
    info!("Listing AppService contracts (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let user_id = extract_user_id_from_jwt(&jwt)?;

    let client = create_client()?;

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Query for AppService contracts (try provider first)
    let payload = json!({
        "userId": user_id,
        "actAs": [party_provider],
        "readAs": [party_provider, party_app],
        "templateFilters": [{
            "templateFilter": {
                "value": format!("{}:AppService:AppService", package_id)
            }
        }]
    });

    let response = client
        .post(&format!("{}state/acs", api_url))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "List services failed (HTTP {}): {}",
            status,
            text
        ));
    }

    let contracts: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();

    println!("=== Active AppService Contracts ===\n");

    if contracts.is_empty() {
        println!("No AppService contracts found.");
        return Ok(());
    }

    for contract in &contracts {
        if let Some(active) = contract.get("activeContract") {
            let cid = active
                .pointer("/contract/contractId")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let app = active
                .pointer("/contract/payload/app")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let provider = active
                .pointer("/contract/payload/provider")
                .and_then(|v| v.as_str())
                .unwrap_or("?");

            println!("Contract ID: {}", cid);
            println!("  App: {}", app);
            println!("  Provider: {}", provider);
            println!();
        }
    }

    Ok(())
}

/// List pending AppServiceRequest contracts
pub async fn handle_list_service_requests() -> Result<()> {
    info!("Listing AppServiceRequest contracts (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_app = std::env::var("PARTY_APP")
        .map_err(|_| anyhow::anyhow!("PARTY_APP not set"))?;
    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let user_id = extract_user_id_from_jwt(&jwt)?;

    let client = create_client()?;

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    // Query for AppServiceRequest contracts
    let payload = json!({
        "userId": user_id,
        "actAs": [party_provider],
        "readAs": [party_provider, party_app],
        "templateFilters": [{
            "templateFilter": {
                "value": format!("{}:AppServiceRequest:AppServiceRequest", package_id)
            }
        }]
    });

    let response = client
        .post(&format!("{}state/acs", api_url))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "List service requests failed (HTTP {}): {}",
            status,
            text
        ));
    }

    let contracts: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();

    println!("=== Pending AppServiceRequest Contracts ===\n");

    if contracts.is_empty() {
        println!("No AppServiceRequest contracts found.");
        return Ok(());
    }

    for contract in &contracts {
        if let Some(active) = contract.get("activeContract") {
            let cid = active
                .pointer("/contract/contractId")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let app = active
                .pointer("/contract/payload/app")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let provider = active
                .pointer("/contract/payload/provider")
                .and_then(|v| v.as_str())
                .unwrap_or("?");

            println!("Contract ID: {}", cid);
            println!("  App: {}", app);
            println!("  Provider: {}", provider);
            println!();
        }
    }

    Ok(())
}

/// Terminate an AppService (provider action)
/// Provider is an internal party, so we use standard submission (not interactive)
pub async fn handle_terminate_service(service_cid: String) -> Result<()> {
    info!(service_cid = %service_cid, "Terminating AppService (devnet)");

    let api_url = std::env::var("LEDGER_API_URL")
        .map_err(|_| anyhow::anyhow!("LEDGER_API_URL not set"))?;
    let jwt = std::env::var("JWT").map_err(|_| anyhow::anyhow!("JWT not set"))?;

    let party_provider = std::env::var("PARTY_PROVIDER")
        .map_err(|_| anyhow::anyhow!("PARTY_PROVIDER not set"))?;
    let package_id = std::env::var("ADVANCED_PAYMENT_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("ADVANCED_PAYMENT_PACKAGE_ID not set"))?;

    let user_id = extract_user_id_from_jwt(&jwt)?;

    let client = create_client()?;
    let template_id = format!("{}:AppService:AppService", package_id);

    // Ensure API URL ends with /
    let api_url = if api_url.ends_with('/') {
        api_url
    } else {
        format!("{}/", api_url)
    };

    let command_id = format!("cmd-{}", chrono::Utc::now().timestamp_millis());

    // Provider is an internal party - use standard submission (not interactive)
    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": service_cid,
                "choice": "AppService_Terminate",
                "choiceArgument": {}
            }
        }],
        "userId": user_id,
        "commandId": command_id,
        "actAs": [party_provider],
        "readAs": [party_provider]
    });

    debug!(
        "Terminate service payload: {}",
        serde_json::to_string_pretty(&payload)?
    );

    let response = client
        .post(&format!("{}commands/submit-and-wait", api_url))
        .bearer_auth(&jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Terminate service failed (HTTP {}): {}",
            status,
            text
        ));
    }

    info!(response = %text, "AppService terminated");

    let response_json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    println!("AppService terminated successfully!");
    println!("Update ID: {}", update_id);
    Ok(())
}

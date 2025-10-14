use anyhow::Result;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use tracing::{info, debug};

/// Create a TestTokenFactory contract
async fn create_factory(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    package_id: &str,
    synchronizer_id: &str,
) -> Result<String> {
    let cmdid = format!("create-factory-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TestToken:TestTokenFactory", package_id);

    info!("Creating TestTokenFactory contract");

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "issuer": party_app_user,
                    "instrumentId": "TestToken"
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": [],
        "workflowId": "TestTokenFactory",
        "synchronizerId": synchronizer_id
    });

    debug!("Create factory payload: {}", serde_json::to_string_pretty(&create_payload)?);

    let create_response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(jwt)
        .json(&create_payload)
        .send()
        .await?;

    let create_status = create_response.status();
    let create_text = create_response.text().await?;

    if !create_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to create TestTokenFactory: HTTP {} - {}",
            create_status,
            create_text
        ));
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    let create_update_id = create_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in create response"))?;

    info!("TestTokenFactory created, updateId: {}", create_update_id);

    // Get the contract ID from the update
    let update_payload = json!({
        "actAs": [party_app_user],
        "updateId": create_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        party_app_user: {
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

    let update_status = update_response.status();
    let update_text = update_response.text().await?;

    if !update_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch update: HTTP {} - {}",
            update_status,
            update_text
        ));
    }

    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract factory contract ID
    let mut factory_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TestToken:TestTokenFactory") {
                        factory_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = factory_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TestTokenFactory contract in create update"))?;

    info!("TestTokenFactory contract ID: {}", cid);

    Ok(cid)
}

/// Mint tokens by exercising the Mint choice
async fn mint_tokens(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    receiver: &str,
    amount: &str,
    package_id: &str,
    factory_cid: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("mint-tokens-{}", chrono::Utc::now().timestamp_millis());
    let template_id = format!("{}:TestToken:TestTokenFactory", package_id);

    info!("Minting {} tokens for {}", amount, receiver);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": factory_cid,
                "choice": "Mint",
                "choiceArgument": {
                    "receiver": receiver,
                    "amount": amount
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user, receiver],
        "workflowId": "MintTestToken",
        "synchronizerId": synchronizer_id
    });

    debug!("Mint payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to mint tokens: HTTP {} - {}",
            status,
            text
        ));
    }

    let json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    info!("Tokens minted, updateId: {}", update_id);

    // Fetch the full update
    let update_payload = json!({
        "actAs": [party_app_user],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        party_app_user: {
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

    // Extract TestToken contract ID
    let mut token_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TestToken:TestToken") && !template.contains("Factory") {
                        token_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = token_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TestToken contract in mint update"))?;

    info!("TestToken contract ID: {}", cid);

    Ok((cid, update_json))
}

pub async fn handle_mint() -> Result<()> {
    info!("Minting TestToken");

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let party_bank = std::env::var("PARTY_BANK")
        .map_err(|_| anyhow::anyhow!("PARTY_BANK not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set in environment"))?;

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Create TestTokenFactory
    let factory_cid = create_factory(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &package_id,
        &synchronizer_id,
    ).await?;

    println!("\n=== TestTokenFactory Created ===");
    println!("Factory Contract ID: {}", factory_cid);

    // Mint 10000 tokens to PARTY_APP_USER
    let (app_user_token_cid, app_user_update) = mint_tokens(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_app_user,
        "10000.0",
        &package_id,
        &factory_cid,
        &synchronizer_id,
    ).await?;

    println!("\n=== Mint 10000 TestToken to PARTY_APP_USER Update ===");
    println!("{}", serde_json::to_string_pretty(&app_user_update)?);

    println!("\n=== Minted 10000 TestToken to PARTY_APP_USER ===");
    println!("Token Contract ID: {}", app_user_token_cid);

    // Mint 1000000 tokens to PARTY_BANK
    let (bank_token_cid, bank_update) = mint_tokens(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_bank,
        "1000000.0",
        &package_id,
        &factory_cid,
        &synchronizer_id,
    ).await?;

    println!("\n=== Mint 1000000 TestToken to PARTY_BANK Update ===");
    println!("{}", serde_json::to_string_pretty(&bank_update)?);

    println!("\n=== Minted 1000000 TestToken to PARTY_BANK ===");
    println!("Token Contract ID: {}", bank_token_cid);

    // Write mint.env file with metadata
    let env_content = format!(
        "# TestToken Mint Metadata - Generated at {}\n\
        # Package ID\n\
        HASH_PACKAGE_ID={}\n\n\
        # Synchronizer ID\n\
        SYNCHRONIZER_ID={}\n\n\
        # Factory Contract\n\
        FACTORY_CONTRACT_ID={}\n\
        FACTORY_TEMPLATE_ID={}:TestToken:TestTokenFactory\n\n\
        # PARTY_APP_USER Token (10000 tokens)\n\
        APP_USER_TOKEN_CONTRACT_ID={}\n\
        APP_USER_TOKEN_TEMPLATE_ID={}:TestToken:TestToken\n\
        APP_USER_TOKEN_AMOUNT=10000.0\n\
        APP_USER_TOKEN_OWNER={}\n\
        APP_USER_TOKEN_ISSUER={}\n\
        APP_USER_TOKEN_INSTRUMENT_ID=TestToken\n\n\
        # PARTY_BANK Token (1000000 tokens)\n\
        BANK_TOKEN_CONTRACT_ID={}\n\
        BANK_TOKEN_TEMPLATE_ID={}:TestToken:TestToken\n\
        BANK_TOKEN_AMOUNT=1000000.0\n\
        BANK_TOKEN_OWNER={}\n\
        BANK_TOKEN_ISSUER={}\n\
        BANK_TOKEN_INSTRUMENT_ID=TestToken\n\n\
        # Parties\n\
        PARTY_APP_USER={}\n\
        PARTY_BANK={}\n",
        chrono::Utc::now().to_rfc3339(),
        package_id,
        synchronizer_id,
        factory_cid,
        package_id,
        app_user_token_cid,
        package_id,
        party_app_user,
        party_app_user,
        bank_token_cid,
        package_id,
        party_bank,
        party_app_user,
        party_app_user,
        party_bank,
    );

    let mut file = File::create("mint.env")?;
    file.write_all(env_content.as_bytes())?;

    println!("\n=== mint.env file written successfully ===");
    println!("Location: ./mint.env");

    // Print summary
    println!("\n=== Mint Summary ===");
    println!("Factory CID: {}", factory_cid);
    println!("APP_USER Token CID: {} (10000.0 tokens)", app_user_token_cid);
    println!("BANK Token CID: {} (1000000.0 tokens)", bank_token_cid);
    println!("\nAll metadata written to mint.env");

    Ok(())
}

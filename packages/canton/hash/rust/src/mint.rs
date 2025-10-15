use anyhow::Result;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use tracing::{info, debug};
use uuid::Uuid;

/// Create a TestTokenBurnMintFactory contract
async fn create_burn_mint_factory(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    admin: &str,
    instrument_id: &str,
    package_id: &str,
    synchronizer_id: &str,
) -> Result<String> {
    let cmdid = format!("create-burnmint-factory-{}", chrono::Utc::now().timestamp());
    let template_id = format!("{}:TestToken:TestTokenBurnMintFactory", package_id);

    info!("Creating TestTokenBurnMintFactory contract");

    let create_payload = json!({
        "commands": [{
            "CreateCommand": {
                "templateId": template_id,
                "createArguments": {
                    "admin": admin,
                    "instrumentId": instrument_id
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [admin],
        "readAs": [],
        "workflowId": "TestTokenBurnMintFactory",
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
            "Failed to create TestTokenBurnMintFactory: HTTP {} - {}",
            create_status,
            create_text
        ));
    }

    let create_json: serde_json::Value = serde_json::from_str(&create_text)?;
    let create_update_id = create_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in create response"))?;

    info!("TestTokenBurnMintFactory created, updateId: {}", create_update_id);

    // Get the contract ID from the update
    let update_payload = json!({
        "actAs": [admin],
        "updateId": create_update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        admin: {
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
                    if template.contains(":TestToken:TestTokenBurnMintFactory") {
                        factory_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = factory_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TestTokenBurnMintFactory contract in create update"))?;

    info!("TestTokenBurnMintFactory contract ID: {}", cid);

    Ok(cid)
}

/// Propose mint using propose-accept pattern (only requires admin authorization)
async fn propose_mint_tokens(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    admin: &str,
    receiver: &str,
    amount: &str,
    package_id: &str,
    factory_cid: &str,
    synchronizer_id: &str,
) -> Result<String> {
    let cmdid = format!("propose-mint-{}", chrono::Utc::now().timestamp_millis());
    let template_id = format!("{}:TestToken:TestTokenBurnMintFactory", package_id);

    info!("Proposing mint of {} tokens for {} using ProposeMint", amount, receiver);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": factory_cid,
                "choice": "ProposeMint",
                "choiceArgument": {
                    "receiver": receiver,
                    "amount": amount
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [admin],
        "readAs": [],
        "workflowId": "ProposeMintTestToken",
        "synchronizerId": synchronizer_id
    });

    debug!("ProposeMint payload: {}", serde_json::to_string_pretty(&payload)?);

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
            "Failed to propose mint: HTTP {} - {}",
            status,
            text
        ));
    }

    let response_json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in propose mint response"))?;

    info!("Mint proposed, updateId: {}", update_id);

    // Get the TestTokenMintRequest contract ID from the update
    let update_payload = json!({
        "actAs": [admin],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        admin: {
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

    let update_status = update_response.status();
    let update_text = update_response.text().await?;

    if !update_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch propose mint update: HTTP {} - {}",
            update_status,
            update_text
        ));
    }

    let update_json: serde_json::Value = serde_json::from_str(&update_text)?;

    // Extract TestTokenMintRequest contract ID
    let mut request_cid = None;
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(":TestToken:TestTokenMintRequest") {
                        request_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = request_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TestTokenMintRequest contract in propose mint update"))?;

    info!("TestTokenMintRequest contract ID: {}", cid);

    Ok(cid)
}

/// Accept mint request (requires receiver authorization)
async fn accept_mint_request(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    receiver: &str,
    package_id: &str,
    request_cid: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("accept-mint-{}", chrono::Utc::now().timestamp_millis());
    let template_id = format!("{}:TestToken:TestTokenMintRequest", package_id);

    info!("Accepting mint request {} by {}", request_cid, receiver);

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
        "actAs": [receiver],
        "readAs": [],
        "workflowId": "AcceptMintTestToken",
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
            "Failed to accept mint request: HTTP {} - {}",
            status,
            text
        ));
    }

    let response_json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in accept response"))?;

    info!("Mint request accepted, updateId: {}", update_id);

    // Get the TestToken contract ID from the update
    let update_payload = json!({
        "actAs": [receiver],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        receiver: {
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

    let update_status = update_response.status();
    let update_text = update_response.text().await?;

    if !update_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch accept update: HTTP {} - {}",
            update_status,
            update_text
        ));
    }

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
                    if template.contains(":TestToken:TestToken") && !template.contains("Factory") && !template.contains("Request") {
                        token_cid = created.pointer("/contractId").and_then(|v| v.as_str()).map(String::from);
                        break;
                    }
                }
            }
        }
    }

    let cid = token_cid
        .ok_or_else(|| anyhow::anyhow!("Could not find TestToken contract in accept update"))?;

    info!("TestToken contract ID: {}", cid);

    Ok((cid, update_json))
}

/// Mint tokens using BurnMintFactory_BurnMint choice (burn [] -> mint outputs)
/// This is the direct mint method that requires both admin and receiver authorization.
/// Currently unused - we use propose-accept pattern for all mints.
#[allow(dead_code)]
async fn mint_tokens(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    admin: &str,
    receiver: &str,
    amount: &str,
    package_id: &str,
    factory_cid: &str,
    _instrument_id: &str,
    synchronizer_id: &str,
) -> Result<(String, serde_json::Value)> {
    let cmdid = format!("burnmint-tokens-{}", chrono::Utc::now().timestamp_millis());
    let template_id = format!("{}:TestToken:TestTokenBurnMintFactory", package_id);

    info!("Minting {} tokens for {} using BurnMintFactory", amount, receiver);

    // Build extra actors list (empty since we'll use actAs properly)
    let extra_actors: Vec<String> = vec![];

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": template_id,
                "contractId": factory_cid,
                "choice": "Mint",
                "choiceArgument": {
                    "receiver": receiver,
                    "amount": amount,
                    "extraActors": extra_actors
                }
            }
        }],
        "commandId": cmdid,
        "actAs": [admin, receiver],
        "readAs": [],
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

    let response_json: serde_json::Value = serde_json::from_str(&text)?;
    let update_id = response_json
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in mint response"))?;

    info!("Tokens minted, updateId: {}", update_id);

    // Get the contract ID from the update
    let update_payload = json!({
        "actAs": [admin],
        "updateId": update_id,
        "updateFormat": {
            "includeTransactions": {
                "eventFormat": {
                    "filtersByParty": {
                        admin: {
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

    let update_status = update_response.status();
    let update_text = update_response.text().await?;

    if !update_status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch mint update: HTTP {} - {}",
            update_status,
            update_text
        ));
    }

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

/// Print all updates from the ledger for debugging
async fn print_all_updates(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party: &str,
) -> Result<()> {
    println!("\n=== Fetching All Ledger Updates ===");

    let payload = json!({
        "beginExclusive": "0",
        "endInclusive": "100",
        "actAs": [party],
        "verbose": true,
        "filter": {
            "filtersByParty": {
                party: {}
            }
        }
    });

    let response = client
        .post(&format!("{}v2/updates", api_url))
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch updates: HTTP {} - {}",
            status,
            text
        ));
    }

    let updates: serde_json::Value = serde_json::from_str(&text)?;
    println!("{}", serde_json::to_string_pretty(&updates)?);

    Ok(())
}

/// Mint tokens to a party using existing factory from mint.env
async fn mint_to_party_from_env(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    admin: &str,
    receiver: &str,
    amount: &str,
    package_id: &str,
    synchronizer_id: &str,
) -> Result<()> {
    info!("Minting {} tokens to {} using existing factory from mint.env", amount, receiver);

    // Read mint.env to get factory contract ID
    let env_path = std::path::Path::new("./mint.env");
    if !env_path.exists() {
        return Err(anyhow::anyhow!(
            "mint.env not found. Please run 'cargo run -- mint' first to create the factory."
        ));
    }

    let env_content = std::fs::read_to_string(env_path)?;

    // Parse factory contract ID from mint.env
    let factory_cid = env_content
        .lines()
        .find(|line| line.starts_with("FACTORY_CONTRACT_ID="))
        .and_then(|line| line.strip_prefix("FACTORY_CONTRACT_ID="))
        .ok_or_else(|| anyhow::anyhow!("FACTORY_CONTRACT_ID not found in mint.env"))?
        .trim()
        .to_string();

    info!("Using factory contract: {}", factory_cid);

    println!("\n=== Proposing Mint {} TestToken to {} ===", amount, receiver);

    // Propose mint using propose-accept pattern
    let request_cid = propose_mint_tokens(
        client,
        api_url,
        jwt,
        admin,
        receiver,
        amount,
        package_id,
        &factory_cid,
        synchronizer_id,
    ).await?;

    println!("TestTokenMintRequest Contract ID: {}", request_cid);
    println!("Mint request proposed successfully. Now {} needs to accept it.", receiver);

    println!("\n=== Accepting Mint Request as {} ===", receiver);
    let (token_cid, update) = accept_mint_request(
        client,
        api_url,
        jwt,
        receiver,
        package_id,
        &request_cid,
        synchronizer_id,
    ).await?;

    println!("\n=== Mint Update ===");
    println!("{}", serde_json::to_string_pretty(&update)?);

    println!("\n=== Successfully Minted {} TestToken to {} ===", amount, receiver);
    println!("Token Contract ID: {}", token_cid);
    println!("\nThis creates a new holding contract that adds to existing holdings.");
    println!("To see total balance, run: make tokens");

    Ok(())
}

pub async fn handle_mint(print_ledger: bool, party: Option<String>, amount: Option<String>) -> Result<()> {
    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let synchronizer_id = std::env::var("SYNCHRONIZER_ID")
        .map_err(|_| anyhow::anyhow!("SYNCHRONIZER_ID not set in environment"))?;

    // Generate random instrumentId
    let instrument_id = format!("TestToken-{}", Uuid::new_v4());

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Print all ledger updates if requested
    if print_ledger {
        print_all_updates(&client, &api_url, &jwt, &party_app_user).await?;
    }

    // If --party and --amount are provided, mint to that party using existing factory
    if let (Some(target_party), Some(mint_amount)) = (party, amount) {
        return mint_to_party_from_env(
            &client,
            &api_url,
            &jwt,
            &party_app_user,
            &target_party,
            &mint_amount,
            &package_id,
            &synchronizer_id,
        ).await;
    }

    // Otherwise, perform full initial mint setup
    info!("Minting TestToken using BurnMintFactory");

    let party_bank = std::env::var("PARTY_BANK")
        .map_err(|_| anyhow::anyhow!("PARTY_BANK not set in environment"))?;

    // Create TestTokenBurnMintFactory
    let factory_cid = create_burn_mint_factory(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &instrument_id,
        &package_id,
        &synchronizer_id,
    ).await?;

    println!("\n=== TestTokenBurnMintFactory Created ===");
    println!("Factory Contract ID: {}", factory_cid);

    let party_holder = std::env::var("PARTY_HOLDER")
        .map_err(|_| anyhow::anyhow!("PARTY_HOLDER not set in environment"))?;

    // Mint 10000 tokens to PARTY_HOLDER using propose-accept pattern
    println!("\n=== Proposing Mint 10000 TestToken to PARTY_HOLDER (propose-accept pattern) ===");
    let holder_request_cid = propose_mint_tokens(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &party_holder,
        "10000.0",
        &package_id,
        &factory_cid,
        &synchronizer_id,
    ).await?;

    println!("TestTokenMintRequest Contract ID: {}", holder_request_cid);
    println!("Mint request proposed successfully. Now PARTY_HOLDER needs to accept it.");

    println!("\n=== Accepting Mint Request as PARTY_HOLDER ===");
    let (holder_token_cid, holder_update) = accept_mint_request(
        &client,
        &api_url,
        &jwt,
        &party_holder,
        &package_id,
        &holder_request_cid,
        &synchronizer_id,
    ).await?;

    println!("\n=== Mint 10000 TestToken to PARTY_HOLDER Update ===");
    println!("{}", serde_json::to_string_pretty(&holder_update)?);

    println!("\n=== Minted 10000 TestToken to PARTY_HOLDER (via propose-accept) ===");
    println!("Token Contract ID: {}", holder_token_cid);

    // Mint 1000000 tokens to PARTY_BANK using propose-accept pattern
    println!("\n=== Proposing Mint 1000000 TestToken to PARTY_BANK (propose-accept pattern) ===");
    let bank_request_cid = propose_mint_tokens(
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

    println!("TestTokenMintRequest Contract ID: {}", bank_request_cid);
    println!("Mint request proposed successfully. Now PARTY_BANK needs to accept it.");

    // For now, we'll accept using the same API (assuming BANK can auth via app_user)
    // In a real multi-party scenario, BANK would accept using their own API/JWT
    println!("\n=== Accepting Mint Request as PARTY_BANK ===");
    let (bank_token_cid, bank_update) = accept_mint_request(
        &client,
        &api_url,
        &jwt,
        &party_bank,
        &package_id,
        &bank_request_cid,
        &synchronizer_id,
    ).await?;

    println!("\n=== Mint 1000000 TestToken to PARTY_BANK Update ===");
    println!("{}", serde_json::to_string_pretty(&bank_update)?);

    println!("\n=== Minted 1000000 TestToken to PARTY_BANK (via propose-accept) ===");
    println!("Token Contract ID: {}", bank_token_cid);

    // Write mint.env file with minimal metadata (only what's needed for subsequent mints)
    let env_content = format!(
        "# TestToken Mint Metadata - Generated at {}\n\
        # Instrument ID (unique identifier for this token)\n\
        INSTRUMENT_ID={}\n\n\
        # BurnMint Factory Contract\n\
        FACTORY_CONTRACT_ID={}\n\
        FACTORY_TEMPLATE_ID={}:TestToken:TestTokenBurnMintFactory\n",
        chrono::Utc::now().to_rfc3339(),
        instrument_id,
        factory_cid,
        package_id,
    );

    let mut file = File::create("./mint.env")?;
    file.write_all(env_content.as_bytes())?;

    println!("\n=== mint.env file written successfully ===");
    println!("Location: ./mint.env");

    println!("\n=== Mint Summary ===");
    println!("Factory CID: {}", factory_cid);
    println!("HOLDER Token CID: {} (10000.0 tokens)", holder_token_cid);
    println!("BANK Token CID: {} (1000000.0 tokens)", bank_token_cid);
    println!("\nAll metadata written to mint.env");

    Ok(())
}

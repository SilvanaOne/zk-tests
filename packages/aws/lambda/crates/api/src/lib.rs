use serde_json;
use thiserror::Error;
use tracing::{info, error, debug};
use std::env;

// Re-export the generated models
pub use api_generated::models::{
    MathRequest, 
    MathResponse, 
    ErrorResponse,
    SuiKeypairRequest,
    SuiKeypairResponse,
    SignMessageRequest,
    SignMessageResponse,
    CreateRegistryRequest,
    CreateRegistryResponse,
    TransactionResponse,
    AddDeveloperRequest,
    UpdateDeveloperRequest,
    RemoveDeveloperRequest,
    AddAgentRequest,
    UpdateAgentRequest,
    RemoveAgentRequest,
    AddAppRequest,
    UpdateAppRequest,
    RemoveAppRequest,
    Chain,
    math_response::Operation,
};

/// API Errors
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Overflow error: {0}")]
    Overflow(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Blockchain error: {0}")]
    Blockchain(String),
}

impl From<ApiError> for ErrorResponse {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::InvalidInput(msg) => ErrorResponse {
                error: "INVALID_INPUT".to_string(),
                message: msg,
            },
            ApiError::Overflow(msg) => ErrorResponse {
                error: "OVERFLOW".to_string(),
                message: msg,
            },
            ApiError::InvalidOperation(msg) => ErrorResponse {
                error: "INVALID_OPERATION".to_string(),
                message: msg,
            },
            ApiError::Json(err) => ErrorResponse {
                error: "JSON_ERROR".to_string(),
                message: err.to_string(),
            },
            ApiError::Blockchain(msg) => ErrorResponse {
                error: "BLOCKCHAIN_ERROR".to_string(),
                message: msg,
            },
        }
    }
}

/// Handler for add operation (async version with blockchain)
pub async fn add_numbers_async(request: MathRequest) -> Result<MathResponse, ApiError> {
    info!("Processing add operation: a={}, b={}", request.a, request.b);
    
    // Convert i64 to u64 for calculation (checking for negative values)
    if request.a < 0 || request.b < 0 {
        error!("Invalid input: negative values detected - a={}, b={}", request.a, request.b);
        return Err(ApiError::InvalidInput("Values must be non-negative".to_string()));
    }
    
    let a = request.a as u64;
    let b = request.b as u64;
    
    // Check if Sui environment variables are configured
    let use_blockchain = env::var("SUI_PACKAGE_ID").is_ok() 
        && env::var("SUI_ADDRESS").is_ok()
        && env::var("SUI_SECRET_KEY").is_ok();
    
    if use_blockchain {
        info!("Using Sui blockchain for calculation");
        // Use Sui blockchain for calculation
        match sui::add::SuiClient::from_env().await {
            Ok(client) => {
                match client.call_add_function(a, b).await {
                    Ok((sum, tx_hash)) => {
                        info!("Successfully computed sum on blockchain: sum={}, tx_hash={}", sum, tx_hash);
                        
                        // Check if result fits in i64
                        if sum > i64::MAX as u64 {
                            error!(result = %sum, max = %i64::MAX, "Result exceeds i64 maximum");
                            return Err(ApiError::Overflow("Result exceeds i64 maximum".to_string()));
                        }
                        
                        let mut response = MathResponse::new(
                            sum as i64,
                            Operation::Add,
                        );
                        response.tx_hash = Some(tx_hash);
                        
                        info!("Add operation successful: result={}", sum);
                        return Ok(response);
                    },
                    Err(e) => {
                        error!("Blockchain computation failed: {}", e);
                        return Err(ApiError::Blockchain(format!("Blockchain error: {}", e)));
                    }
                }
            },
            Err(e) => {
                error!("Failed to initialize Sui client: {}", e);
                return Err(ApiError::Blockchain(format!("Failed to initialize blockchain client: {}", e)));
            }
        }
    } else {
        info!("Using local computation (blockchain not configured)");
        // Local computation
        let result = a.checked_add(b)
            .ok_or_else(|| {
                error!("Addition overflow detected: {} + {}", a, b);
                ApiError::Overflow(format!(
                    "Addition overflow: {} + {} exceeds u64 maximum",
                    a, b
                ))
            })?;
        
        // Check if result fits in i64 (for the generated model)
        if result > i64::MAX as u64 {
            error!("Result {} exceeds i64 maximum {}", result, i64::MAX);
            return Err(ApiError::Overflow("Result exceeds i64 maximum".to_string()));
        }
        
        info!("Add operation successful (local): result={}", result);
        Ok(MathResponse::new(
            result as i64,
            Operation::Add,
        ))
    }
}

/// Handler for add operation (sync wrapper for compatibility)
pub fn add_numbers(request: MathRequest) -> Result<MathResponse, ApiError> {
    // Use the existing tokio runtime if available, otherwise create a new one
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're already in a tokio runtime, use it
            handle.block_on(add_numbers_async(request))
        },
        Err(_) => {
            // No runtime exists, create one
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| ApiError::Blockchain(format!("Failed to create runtime: {}", e)))?;
            runtime.block_on(add_numbers_async(request))
        }
    }
}

/// Handler for multiply operation
pub fn multiply_numbers(request: MathRequest) -> Result<MathResponse, ApiError> {
    info!("Processing multiply operation");
    
    // Convert i64 to u64 for calculation (checking for negative values)
    if request.a < 0 || request.b < 0 {
        error!("Invalid input: negative values detected - a={}, b={}", request.a, request.b);
        return Err(ApiError::InvalidInput("Values must be non-negative".to_string()));
    }
    
    let a = request.a as u64;
    let b = request.b as u64;
    
    let result = a.checked_mul(b)
        .ok_or_else(|| {
            error!("Multiplication overflow detected: {} * {}", a, b);
            ApiError::Overflow(format!(
                "Multiplication overflow: {} * {} exceeds u64 maximum",
                a, b
            ))
        })?;
    
    // Check if result fits in i64 (for the generated model)
    if result > i64::MAX as u64 {
        error!(result = %result, max = %i64::MAX, "Result exceeds i64 maximum");
        return Err(ApiError::Overflow("Result exceeds i64 maximum".to_string()));
    }
    
    info!("Multiply operation successful: result={}", result);
    Ok(MathResponse::new(
        result as i64,
        Operation::Multiply,
    ))
}

/// Handler for generating or retrieving a Sui keypair (async version)
pub async fn generate_sui_keypair_async(request: SuiKeypairRequest) -> Result<SuiKeypairResponse, ApiError> {
    info!("Processing Sui keypair request for {}:{}", request.login_type, request.login);
    
    // Get configuration from environment
    let table_name = env::var("KEYPAIRS_TABLE_NAME")
        .unwrap_or_else(|_| "sui-keypairs".to_string());
    let kms_key_id = env::var("KMS_KEY_ID")
        .map_err(|_| ApiError::InvalidOperation("KMS_KEY_ID not configured".to_string()))?;
    
    // Initialize secure storage
    let storage = db::secure_storage::SecureKeyStorage::new(table_name, kms_key_id).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to initialize secure storage: {}", e)))?;
    
    // Check if keypair already exists
    if let Some(address) = storage.get_keypair_address(&request.login_type, &request.login).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to check existing keypair: {}", e)))? {
        info!("Retrieved existing Sui keypair for {}:{} with address: {}", 
              request.login_type, request.login, address);
        return Ok(SuiKeypairResponse::new(address));
    }
    
    // Generate new keypair
    info!("Generating new Sui keypair for {}:{}", request.login_type, request.login);
    let keypair = sui::keypair::generate_ed25519();
    let address = keypair.address.to_string();
    
    // Store the new keypair
    storage.store_new_keypair(
        &request.login_type, 
        &request.login, 
        &address,
        &keypair.sui_private_key
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to store keypair: {}", e)))?;
    
    info!("Generated new Sui keypair for {}:{} with address: {}", 
          request.login_type, request.login, address);
    
    Ok(SuiKeypairResponse::new(address))
}

/// Handler for generating or retrieving a Sui keypair (sync wrapper)
pub fn generate_sui_keypair(request: SuiKeypairRequest) -> Result<SuiKeypairResponse, ApiError> {
    // Use the existing tokio runtime if available, otherwise create a new one
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're already in a tokio runtime, use it
            handle.block_on(generate_sui_keypair_async(request))
        },
        Err(_) => {
            // No runtime exists, create one
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| ApiError::Blockchain(format!("Failed to create runtime: {}", e)))?;
            runtime.block_on(generate_sui_keypair_async(request))
        }
    }
}

/// Handler for signing a message with a Sui keypair (async version)
pub async fn sign_message_async(request: SignMessageRequest) -> Result<SignMessageResponse, ApiError> {
    info!("Processing sign message request for {}:{}", request.login_type, request.login);
    
    // Get configuration from environment
    let table_name = env::var("KEYPAIRS_TABLE_NAME")
        .unwrap_or_else(|_| "sui-keypairs".to_string());
    let kms_key_id = env::var("KMS_KEY_ID")
        .map_err(|_| ApiError::InvalidOperation("KMS_KEY_ID not configured".to_string()))?;
    
    // Initialize secure storage
    let storage = db::secure_storage::SecureKeyStorage::new(table_name, kms_key_id).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to initialize secure storage: {}", e)))?;
    
    // Get the address first to check if keypair exists
    let address = storage.get_keypair_address(&request.login_type, &request.login).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to check keypair: {}", e)))?
        .ok_or_else(|| ApiError::InvalidInput(format!("Keypair not found for {}:{}", request.login_type, request.login)))?;
    
    // Get the private key
    let sui_private_key = storage.get_private_key(&request.login_type, &request.login).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to retrieve private key: {}", e)))?;
    
    // Parse the private key to get the secret key bytes
    let secret_key = sui::keypair::parse_sui_private_key(&sui_private_key)
        .map_err(|e| ApiError::Blockchain(format!("Failed to parse private key: {}", e)))?;
    
    // Convert message from Vec<i32> to Vec<u8>
    let message_bytes: Vec<u8> = request.message.iter()
        .map(|&b| b as u8)
        .collect();
    
    // Sign the message
    let signature = sui::keypair::sign_message(&secret_key, &message_bytes);
    
    // Verify the signature and measure the time
    let sui_address = sui::keypair::parse_address(&address)
        .map_err(|e| ApiError::Blockchain(format!("Failed to parse address: {}", e)))?;
    
    let start_verify = std::time::Instant::now();
    let verification_result = sui::keypair::verify_with_address(&sui_address, &message_bytes, &signature);
    let verify_elapsed_ms = start_verify.elapsed().as_millis();
    
    info!("Signature verification result: {} (took {}ms)", verification_result, verify_elapsed_ms);
    
    if !verification_result {
        return Err(ApiError::Blockchain("Signature verification failed".to_string()));
    }
    
    // Convert signature to Vec<i32> for response
    let signature_response: Vec<i32> = signature.iter()
        .map(|&b| b as i32)
        .collect();
    
    info!("Successfully signed message for {}:{} with address: {}", 
          request.login_type, request.login, address);
    
    Ok(SignMessageResponse::new(signature_response, address))
}

/// Handler for signing a message with a Sui keypair (sync wrapper)
pub fn sign_message(request: SignMessageRequest) -> Result<SignMessageResponse, ApiError> {
    // Use the existing tokio runtime if available, otherwise create a new one
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're already in a tokio runtime, use it
            handle.block_on(sign_message_async(request))
        },
        Err(_) => {
            // No runtime exists, create one
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| ApiError::Blockchain(format!("Failed to create runtime: {}", e)))?;
            runtime.block_on(sign_message_async(request))
        }
    }
}

/// Get RPC URL and registry package for a chain
fn get_chain_config(chain: Chain) -> Result<(String, String, String), ApiError> {
    match chain {
        Chain::Devnet => {
            let package = env::var("SILVANA_REGISTRY_PACKAGE")
                .or_else(|_| env::var("SILVANA_REGISTRY_PACKAGE_DEVNET"))
                .map_err(|_| ApiError::InvalidOperation("Registry package not configured for devnet".to_string()))?;
            Ok(("https://fullnode.devnet.sui.io:443".to_string(), package, "devnet".to_string()))
        },
        Chain::Testnet => {
            let package = env::var("SILVANA_REGISTRY_PACKAGE_TESTNET")
                .map_err(|_| ApiError::InvalidOperation("Registry package not configured for testnet".to_string()))?;
            Ok(("https://fullnode.testnet.sui.io:443".to_string(), package, "testnet".to_string()))
        },
        Chain::Mainnet => {
            let package = env::var("SILVANA_REGISTRY_PACKAGE_MAINNET")
                .map_err(|_| ApiError::InvalidOperation("Registry package not configured for mainnet".to_string()))?;
            Ok(("https://fullnode.mainnet.sui.io:443".to_string(), package, "mainnet".to_string()))
        },
    }
}

/// Handler for creating a Silvana registry
pub async fn create_registry_async(request: CreateRegistryRequest) -> Result<CreateRegistryResponse, ApiError> {
    info!("Creating Silvana registry: name={}, chain={:?}", request.name, request.chain);
    
    // Get chain-specific configuration
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    // Call create_registry function
    let result = sui::create_registry(
        &rpc_url,
        &registry_package,
        request.name,
        &chain_name,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to create registry: {}", e)))?;
    
    info!("Successfully created registry: id={}, tx_digest={}", result.registry_id, result.tx_digest);
    
    Ok(CreateRegistryResponse::new(
        result.registry_id,
        result.tx_digest,
        result.admin_address,
    ))
}

/// Handler for adding a developer
pub async fn add_developer_async(request: AddDeveloperRequest) -> Result<TransactionResponse, ApiError> {
    info!("Adding developer: name={}, github={}", request.name, request.github);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::add_developer(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
        request.github,
        request.image.flatten(),
        request.description.flatten(),
        request.site.flatten(),
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to add developer: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for updating a developer
pub async fn update_developer_async(request: UpdateDeveloperRequest) -> Result<TransactionResponse, ApiError> {
    info!("Updating developer: name={}, github={}", request.name, request.github);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::update_developer(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
        request.github,
        request.image.flatten(),
        request.description.flatten(),
        request.site.flatten(),
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to update developer: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for removing a developer
pub async fn remove_developer_async(request: RemoveDeveloperRequest) -> Result<TransactionResponse, ApiError> {
    info!("Removing developer: name={}", request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::remove_developer(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
        request.agent_names,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to remove developer: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for adding an agent
pub async fn add_agent_async(request: AddAgentRequest) -> Result<TransactionResponse, ApiError> {
    info!("Adding agent: developer={}, name={}", request.developer, request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::add_agent(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.developer,
        request.name,
        request.image.flatten(),
        request.description.flatten(),
        request.site.flatten(),
        request.chains,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to add agent: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for updating an agent
pub async fn update_agent_async(request: UpdateAgentRequest) -> Result<TransactionResponse, ApiError> {
    info!("Updating agent: developer={}, name={}", request.developer, request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::update_agent(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.developer,
        request.name,
        request.image.flatten(),
        request.description.flatten(),
        request.site.flatten(),
        request.chains,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to update agent: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for removing an agent
pub async fn remove_agent_async(request: RemoveAgentRequest) -> Result<TransactionResponse, ApiError> {
    info!("Removing agent: developer={}, name={}", request.developer, request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::remove_agent(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.developer,
        request.name,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to remove agent: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for adding an app
pub async fn add_app_async(request: AddAppRequest) -> Result<TransactionResponse, ApiError> {
    info!("Adding app: name={}", request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::add_app(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
        request.description.flatten(),
        request.image.flatten(),
        request.site.flatten(),
        request.app_cap.flatten(),
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to add app: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for updating an app
pub async fn update_app_async(request: UpdateAppRequest) -> Result<TransactionResponse, ApiError> {
    info!("Updating app: name={}", request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::update_app(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
        request.description.flatten(),
        request.image.flatten(),
        request.site.flatten(),
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to update app: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Handler for removing an app
pub async fn remove_app_async(request: RemoveAppRequest) -> Result<TransactionResponse, ApiError> {
    info!("Removing app: name={}", request.name);
    
    let (rpc_url, registry_package, chain_name) = get_chain_config(request.chain)?;
    
    let tx_digest = sui::remove_app(
        &rpc_url,
        &registry_package,
        &request.registry_id,
        &chain_name,
        request.name,
    ).await
        .map_err(|e| ApiError::Blockchain(format!("Failed to remove app: {}", e)))?;
    
    Ok(TransactionResponse::new(tx_digest))
}

/// Process API request based on path (async version)
pub async fn process_request_async(path: &str, body: &str) -> Result<String, ApiError> {
    debug!("Processing request for path: {}", path);
    
    match path {
        "/add" => {
            debug!("Parsing request body for add operation");
            let request: MathRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = add_numbers_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Add operation completed successfully");
            Ok(json)
        },
        "/multiply" => {
            debug!("Parsing request body for multiply operation");
            let request: MathRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = multiply_numbers(request)?;
            let json = serde_json::to_string(&response)?;
            info!("Multiply operation completed successfully");
            Ok(json)
        },
        "/generate-sui-keypair" => {
            debug!("Processing Sui keypair request");
            let request: SuiKeypairRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = generate_sui_keypair_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Sui keypair request completed successfully");
            Ok(json)
        },
        "/sign-message" => {
            debug!("Processing sign message request");
            let request: SignMessageRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = sign_message_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Sign message request completed successfully");
            Ok(json)
        },
        "/create-registry" => {
            debug!("Processing create registry request");
            let request: CreateRegistryRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = create_registry_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Registry creation completed successfully");
            Ok(json)
        },
        "/add-developer" => {
            debug!("Processing add developer request");
            let request: AddDeveloperRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = add_developer_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Add developer completed successfully");
            Ok(json)
        },
        "/update-developer" => {
            debug!("Processing update developer request");
            let request: UpdateDeveloperRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = update_developer_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Update developer completed successfully");
            Ok(json)
        },
        "/remove-developer" => {
            debug!("Processing remove developer request");
            let request: RemoveDeveloperRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = remove_developer_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Remove developer completed successfully");
            Ok(json)
        },
        "/add-agent" => {
            debug!("Processing add agent request");
            let request: AddAgentRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = add_agent_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Add agent completed successfully");
            Ok(json)
        },
        "/update-agent" => {
            debug!("Processing update agent request");
            let request: UpdateAgentRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = update_agent_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Update agent completed successfully");
            Ok(json)
        },
        "/remove-agent" => {
            debug!("Processing remove agent request");
            let request: RemoveAgentRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = remove_agent_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Remove agent completed successfully");
            Ok(json)
        },
        "/add-app" => {
            debug!("Processing add app request");
            let request: AddAppRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = add_app_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Add app completed successfully");
            Ok(json)
        },
        "/update-app" => {
            debug!("Processing update app request");
            let request: UpdateAppRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = update_app_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Update app completed successfully");
            Ok(json)
        },
        "/remove-app" => {
            debug!("Processing remove app request");
            let request: RemoveAppRequest = serde_json::from_str(body)
                .map_err(|e| {
                    error!("Failed to parse request body: {}", e);
                    e
                })?;
            let response = remove_app_async(request).await?;
            let json = serde_json::to_string(&response)?;
            info!("Remove app completed successfully");
            Ok(json)
        },
        _ => {
            error!(path = %path, "Unknown path requested");
            Err(ApiError::InvalidOperation(format!("Unknown path: {}", path)))
        }
    }
}

/// Process API request based on path (sync wrapper for compatibility)
pub fn process_request(path: &str, body: &str) -> Result<String, ApiError> {
    // Use the existing tokio runtime if available, otherwise create a new one
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're already in a tokio runtime, use it
            handle.block_on(process_request_async(path, body))
        },
        Err(_) => {
            // No runtime exists, create one
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|e| ApiError::Blockchain(format!("Failed to create runtime: {}", e)))?;
            runtime.block_on(process_request_async(path, body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let request = MathRequest::new(100, 200);
        let response = add_numbers(request).unwrap();
        assert_eq!(response.result, 300);
        assert_eq!(response.operation, Operation::Add);
    }

    #[test]
    fn test_multiply() {
        let request = MathRequest::new(10, 20);
        let response = multiply_numbers(request).unwrap();
        assert_eq!(response.result, 200);
        assert_eq!(response.operation, Operation::Multiply);
    }

    #[test]
    fn test_add_overflow() {
        let request = MathRequest::new(i64::MAX, 1);
        let result = add_numbers(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_negative_values() {
        let request = MathRequest::new(-1, 10);
        let result = add_numbers(request);
        assert!(result.is_err());
    }
}
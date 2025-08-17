use anyhow::{anyhow, Context, Result};
use std::env;
use std::str::FromStr;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use sui_crypto::SuiSigner;
use tracing::{info, debug, error};
use db::KeyLock;
use crate::chain::{load_sender_from_env, get_reference_gas_price, pick_gas_object};

const SUI_CLOCK_OBJECT_ID: &str = "0x0000000000000000000000000000000000000000000000000000000000000006";

// Define Move types for serialization
#[derive(serde::Serialize)]
struct MoveString {
    bytes: Vec<u8>,
}

#[derive(serde::Serialize)]
struct MoveOption<T> {
    vec: Vec<T>,
}

/// Creates a Silvana registry on the Sui blockchain
pub async fn create_registry(
    rpc_url: &str,
    registry_package: &str,
    name: String,
    chain: &str,
) -> Result<CreateRegistryResult> {
    debug!("Creating registry '{}' on chain '{}' with package '{}'", name, chain, registry_package);
    
    // Get sender info from environment
    let address_str = env::var("SUI_ADDRESS")?;
    
    // Acquire lock before preparing transaction
    let key_lock = KeyLock::new().await?;
    let lock_guard = key_lock.acquire_lock(&address_str, chain).await?;
    info!("Acquired lock for Sui key on {}", chain);
    
    // Parse package ID
    let package_id = sui::Address::from_str(registry_package)
        .context("Failed to parse registry package ID")?;
    
    // Parse sender and secret key
    let (sender, sk) = load_sender_from_env()?;
    
    // Build transaction using TransactionBuilder
    let mut tb = sui_transaction_builder::TransactionBuilder::new();
    tb.set_sender(sender);
    tb.set_gas_budget(100_000_000); // 0.1 SUI
    
    // Get gas price
    let mut price_client = GrpcClient::new(rpc_url.to_string())?;
    let gas_price = get_reference_gas_price(&mut price_client).await?;
    tb.set_gas_price(gas_price);
    
    // Select gas object
    let mut coin_client = GrpcClient::new(rpc_url.to_string())?;
    let gas_ref = pick_gas_object(&mut coin_client, sender).await?;
    let gas_input = sui_transaction_builder::unresolved::Input::owned(
        *gas_ref.object_id(),
        gas_ref.version(),
        *gas_ref.digest(),
    );
    tb.add_gas_objects(vec![gas_input]);
    debug!("Gas object added: {:?}", gas_ref);
    
    // Create the string argument for the registry name
    let move_string = MoveString {
        bytes: name.into_bytes(),
    };
    let name_arg = tb.input(sui_transaction_builder::Serialized(&move_string));
    
    // Function call: coordination::registry::create_registry(String)
    let func = sui_transaction_builder::Function::new(
        package_id,
        "registry".parse()
            .map_err(|e| anyhow!("Failed to parse module name 'registry': {}", e))?,
        "create_registry".parse()
            .map_err(|e| anyhow!("Failed to parse function name 'create_registry': {}", e))?,
        vec![],
    );
    tb.move_call(func, vec![name_arg]);
    
    // Finalize and sign
    let tx = tb.finish()?;
    let sig = sk.sign_transaction(&tx)?;
    
    // Execute transaction via gRPC
    let mut grpc = GrpcClient::new(rpc_url.to_string())?;
    let mut exec = grpc.execution_client();
    let req = proto::ExecuteTransactionRequest {
        transaction: Some(tx.into()),
        signatures: vec![sig.into()],
        read_mask: Some(FieldMask { 
            paths: vec![
                "finality".into(), 
                "transaction".into()
            ] 
        }),
    };
    
    debug!("Sending create_registry transaction...");
    let tx_start = std::time::Instant::now();
    let exec_result = exec.execute_transaction(req).await;
    let tx_elapsed_ms = tx_start.elapsed().as_millis();
    
    // Release lock immediately after transaction is sent
    match lock_guard.release().await {
        Ok(_) => debug!("Lock released successfully"),
        Err(e) => {
            // Log the error but don't fail the transaction since it already succeeded
            debug!("Warning: Failed to release lock cleanly: {}", e);
        }
    }
    
    let resp = match exec_result {
        Ok(r) => r,
        Err(e) => {
            debug!("Transaction execution error: {:?}", e);
            debug!("Error string: {}", e.to_string());
            return Err(anyhow::anyhow!("Failed to execute transaction: {}", e));
        }
    };
    let tx_resp = resp.into_inner();
    
    // Check transaction was successful
    if tx_resp.finality.is_none() {
        return Err(anyhow::anyhow!("Transaction did not achieve finality"));
    }
    
    let tx_digest = tx_resp.transaction
        .as_ref()
        .and_then(|t| t.digest.as_ref())
        .context("Failed to get transaction digest")?
        .to_string();
    
    info!("Transaction executed on blockchain: {} (took {}ms)", tx_digest, tx_elapsed_ms);
    
    // Extract the registry ID from RegistryCreatedEvent
    // Following the pattern from rpc-tx/rpc/src/state.rs
    let registry_id = if let Some(executed) = tx_resp.transaction.as_ref() {
        if let Some(events) = executed.events.as_ref() {
            info!("Transaction has {} events", events.events.len());
            
            // Look for RegistryCreatedEvent
            let mut found_id: Option<String> = None;
            for (idx, e) in events.events.iter().enumerate() {
                debug!("Event[{}]: type={:?}, module={:?}, json_present={}", 
                      idx, e.event_type, e.module, e.json.is_some());
                
                // Check if this is a RegistryCreatedEvent
                if e.event_type
                    .as_deref()
                    .map(|t| t.ends_with("::RegistryCreatedEvent"))
                    .unwrap_or(false)
                {
                    info!("Found RegistryCreatedEvent at index {}", idx);
                    
                    // First try to extract from JSON
                    if let Some(json) = &e.json {
                        if let Some(id) = extract_registry_id_from_json(json.as_ref()) {
                            info!("Extracted registry ID from event JSON: {}", id);
                            found_id = Some(id);
                            break;
                        } else {
                            debug!("Failed to parse registry ID from event JSON");
                        }
                    }
                    
                    // If JSON extraction failed, try BCS
                    if let Some(id) = try_decode_registry_created_from_bcs(e) {
                        info!("Extracted registry ID from event BCS: {}", id);
                        found_id = Some(id);
                        break;
                    }
                }
            }
            
            found_id.unwrap_or_else(|| {
                // Fallback: look for created SilvanaRegistry object in effects
                info!("No RegistryCreatedEvent found, checking created objects");
                executed.effects.as_ref()
                    .and_then(|effects| {
                        effects.changed_objects
                            .iter()
                            .find(|obj| {
                                let is_created = obj.input_state == Some(proto::changed_object::InputObjectState::DoesNotExist as i32);
                                let is_registry = obj.object_type.as_ref()
                                    .map(|t| t.contains("::registry::SilvanaRegistry"))
                                    .unwrap_or(false);
                                
                                if is_created {
                                    info!("Created object: id={:?}, type={:?}, is_registry={}", 
                                          obj.object_id, obj.object_type, is_registry);
                                }
                                
                                is_created && is_registry
                            })
                            .and_then(|obj| {
                                info!("Found created SilvanaRegistry object: {:?}", obj.object_id);
                                obj.object_id.clone()
                            })
                    })
                    .unwrap_or_else(|| {
                        error!("Failed to extract registry ID from transaction");
                        tx_digest.clone()
                    })
            })
        } else {
            error!("No events in transaction response");
            tx_digest.clone()
        }
    } else {
        error!("No transaction in response");
        tx_digest.clone()
    };
    
    Ok(CreateRegistryResult {
        registry_id,
        tx_digest,
        admin_address: sender.to_string(),
    })
}

fn extract_registry_id_from_json(value: &prost_types::Value) -> Option<String> {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::StructValue(s)) => {
            let m = &s.fields;
            // The RegistryCreatedEvent has an 'id' field with the registry address
            m.get("id").and_then(|v| match &v.kind {
                Some(Kind::StringValue(s)) => {
                    debug!("Found registry ID in event: {}", s);
                    Some(s.clone())
                },
                _ => {
                    debug!("Event 'id' field is not a string");
                    None
                }
            })
        }
        _ => {
            debug!("Event JSON is not a struct");
            None
        }
    }
}

// Try to decode RegistryCreatedEvent from BCS if JSON is not available
fn try_decode_registry_created_from_bcs(e: &proto::Event) -> Option<String> {
    let contents = e.contents.as_ref()?;
    let value_ref = contents.value.as_ref()?;
    let value_bytes: &[u8] = value_ref.as_ref();
    
    // RegistryCreatedEvent layout in BCS:
    // - id: address (32 bytes)
    // - name: String (variable length with length prefix)
    // - admin: address (32 bytes)
    
    if value_bytes.len() >= 32 {
        let addr_bytes = &value_bytes[..32];
        if let Ok(addr) = sui::Address::from_bytes(addr_bytes) {
            debug!("Extracted registry ID from BCS: {}", addr);
            return Some(addr.to_string());
        }
    }
    None
}

pub struct CreateRegistryResult {
    pub registry_id: String,
    pub tx_digest: String,
    pub admin_address: String,
}

/// Adds a developer to the registry
pub async fn add_developer(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
    github: String,
    image: Option<String>,
    description: Option<String>,
    site: Option<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "add_developer",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: github.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: image.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: site.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
            ]
        },
    ).await
}

/// Updates a developer in the registry
pub async fn update_developer(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
    github: String,
    image: Option<String>,
    description: Option<String>,
    site: Option<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "update_developer",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: github.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: image.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: site.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
            ]
        },
    ).await
}

/// Removes a developer from the registry
pub async fn remove_developer(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
    agent_names: Vec<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "remove_developer",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(
                    &agent_names.into_iter().map(|s| MoveString { bytes: s.into_bytes() }).collect::<Vec<_>>()
                )),
            ]
        },
    ).await
}

/// Adds an agent to the registry
pub async fn add_agent(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    developer: String,
    name: String,
    image: Option<String>,
    description: Option<String>,
    site: Option<String>,
    chains: Vec<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "add_agent",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: developer.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: image.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: site.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(
                    &chains.into_iter().map(|s| MoveString { bytes: s.into_bytes() }).collect::<Vec<_>>()
                )),
            ]
        },
    ).await
}

/// Updates an agent in the registry
pub async fn update_agent(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    developer: String,
    name: String,
    image: Option<String>,
    description: Option<String>,
    site: Option<String>,
    chains: Vec<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "update_agent",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: developer.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: image.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: site.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
                tb.input(sui_transaction_builder::Serialized(
                    &chains.into_iter().map(|s| MoveString { bytes: s.into_bytes() }).collect::<Vec<_>>()
                )),
            ]
        },
    ).await
}

/// Removes an agent from the registry
pub async fn remove_agent(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    developer: String,
    name: String,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "remove_agent",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: developer.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
            ]
        },
    ).await
}

/// Adds an app to the registry
pub async fn add_app(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
    description: Option<String>,
    _image: Option<String>,
    _site: Option<String>,
    _app_cap: Option<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "add_app",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
            ]
        },
    ).await
}

/// Updates an app in the registry
pub async fn update_app(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
    description: Option<String>,
    _image: Option<String>,
    _site: Option<String>,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "update_app",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
                tb.input(sui_transaction_builder::Serialized(&MoveOption {
                    vec: description.map(|s| MoveString { bytes: s.into_bytes() }).into_iter().collect(),
                })),
            ]
        },
    ).await
}

/// Removes an app from the registry
pub async fn remove_app(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    name: String,
) -> Result<String> {
    execute_registry_function(
        rpc_url,
        registry_package,
        registry_id,
        chain,
        "remove_app",
        |tb| {
            vec![
                tb.input(sui_transaction_builder::Serialized(&MoveString { bytes: name.into_bytes() })),
            ]
        },
    ).await
}

// Helper function to fetch initial shared version of an object
// Based on the working example in rpc-tx/rpc/src/state.rs
async fn fetch_initial_shared_version(rpc_url: &str, object_id: &str) -> Result<u64> {
    info!("Fetching initial shared version for object: {}", object_id);
    
    let mut client = GrpcClient::new(rpc_url.to_string())?;
    let mut ledger = client.ledger_client();
    
    // Retry up to 10 times with 500ms delay, as the object might need time to become shared
    for attempt in 1..=10 {
        info!("Attempt {} to fetch object: {}", attempt, object_id);
        
        let resp = match ledger
            .get_object(proto::GetObjectRequest {
                object_id: Some(object_id.to_string()),
                version: None,
                read_mask: Some(prost_types::FieldMask {
                    paths: vec!["owner".into(), "object_id".into(), "version".into()],
                }),
            })
            .await
        {
            Ok(r) => r.into_inner(),
            Err(status) => {
                info!("Attempt {} - GetObject error: {:?}", attempt, status);
                if attempt < 10 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                } else {
                    return Err(anyhow::anyhow!("Failed to fetch object after 10 attempts: {}", status));
                }
            }
        };
        
        if let Some(obj) = resp.object {
            info!("Attempt {} - object found: id={:?} version={:?} owner={:?}", 
                  attempt, obj.object_id, obj.version, obj.owner);
            
            if let Some(owner) = obj.owner {
                if let Some(kind_i32) = owner.kind {
                    if let Ok(kind_enum) = proto::owner::OwnerKind::try_from(kind_i32) {
                        info!("Owner kind: {:?}", kind_enum);
                        
                        if kind_enum == proto::owner::OwnerKind::Shared {
                            if let Some(v) = owner.version {
                                info!("Found initial shared version: {}", v);
                                return Ok(v);
                            } else {
                                info!("Shared owner but missing initial_shared_version; retrying...");
                            }
                        } else {
                            info!("Owner kind={:?}; waiting for Shared...", kind_enum);
                        }
                    }
                }
            }
        } else {
            info!("Attempt {} - object not found; retrying...", attempt);
        }
        
        if attempt < 10 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    
    Err(anyhow::anyhow!(
        "Object is not shared or missing initial_shared_version after retries"
    ))
}

// Helper function to execute registry management functions
async fn execute_registry_function<F>(
    rpc_url: &str,
    registry_package: &str,
    registry_id: &str,
    chain: &str,
    function_name: &str,
    build_args: F,
) -> Result<String>
where
    F: FnOnce(&mut sui_transaction_builder::TransactionBuilder) -> Vec<sui_sdk_types::Argument>,
{
    info!("Executing {} on chain '{}' with registry '{}'", function_name, chain, registry_id);
    
    // Get sender info from environment
    let address_str = env::var("SUI_ADDRESS")?;
    
    // Acquire lock before preparing transaction
    let key_lock = KeyLock::new().await?;
    let lock_guard = key_lock.acquire_lock(&address_str, chain).await?;
    info!("Acquired lock for Sui key on {}", chain);
    
    // Parse IDs
    info!("Parsing package ID: {}", registry_package);
    let package_id = sui::Address::from_str(registry_package)
        .context("Failed to parse registry package ID")?;
    info!("Parsing registry object ID: {}", registry_id);
    let registry_obj_id = sui::Address::from_str(registry_id)
        .context("Failed to parse registry object ID")?;
    let clock_id = sui::Address::from_str(SUI_CLOCK_OBJECT_ID)
        .context("Failed to parse clock object ID")?;
    
    // Parse sender and secret key
    let (sender, sk) = load_sender_from_env()?;
    
    // Build transaction using TransactionBuilder
    let mut tb = sui_transaction_builder::TransactionBuilder::new();
    tb.set_sender(sender);
    tb.set_gas_budget(100_000_000); // 0.1 SUI
    
    // Get gas price
    let mut price_client = GrpcClient::new(rpc_url.to_string())?;
    let gas_price = get_reference_gas_price(&mut price_client).await?;
    tb.set_gas_price(gas_price);
    
    // Select gas object
    let mut coin_client = GrpcClient::new(rpc_url.to_string())?;
    let gas_ref = pick_gas_object(&mut coin_client, sender).await?;
    let gas_input = sui_transaction_builder::unresolved::Input::owned(
        *gas_ref.object_id(),
        gas_ref.version(),
        *gas_ref.digest(),
    );
    tb.add_gas_objects(vec![gas_input]);
    debug!("Gas object added: {:?}", gas_ref);
    
    // For shared objects created by create_registry, we need to use the correct initial shared version
    // The registry object is created as a shared object in the create_registry function
    // We MUST fetch the actual initial shared version from the object
    let initial_shared_version = fetch_initial_shared_version(rpc_url, registry_id).await
        .context("Failed to fetch initial shared version for registry object")?;
    
    debug!("Registry object initial shared version: {}", initial_shared_version);
    
    // Build function arguments
    // Registry is a shared mutable object with the fetched initial version
    let registry_obj = tb.input(
        sui_transaction_builder::unresolved::Input::shared(
            registry_obj_id,
            initial_shared_version,
            true  // mutable
        )
    );
    
    // Clock is a shared immutable object at a well-known address (0x6)
    // The clock has initial_shared_version of 1
    let clock_obj = tb.input(
        sui_transaction_builder::unresolved::Input::shared(clock_id, 1, false)
    );
    
    // Build arguments - registry first, then function-specific args, then clock
    let mut args = vec![registry_obj];
    let function_args = build_args(&mut tb);
    args.extend(function_args);
    args.push(clock_obj);
    
    // Function call
    let func = sui_transaction_builder::Function::new(
        package_id,
        "registry".parse()
            .map_err(|e| anyhow!("Failed to parse module name 'registry': {}", e))?,
        function_name.parse()
            .map_err(|e| anyhow!("Failed to parse function name '{}': {}", function_name, e))?,
        vec![],
    );
    tb.move_call(func, args);
    
    // Finalize and sign
    let tx = tb.finish()?;
    let sig = sk.sign_transaction(&tx)?;
    
    // Execute transaction via gRPC
    let mut grpc = GrpcClient::new(rpc_url.to_string())?;
    let mut exec = grpc.execution_client();
    let req = proto::ExecuteTransactionRequest {
        transaction: Some(tx.into()),
        signatures: vec![sig.into()],
        read_mask: Some(FieldMask { 
            paths: vec![
                "finality".into(), 
                "transaction".into()
            ] 
        }),
    };
    
    debug!("Sending {} transaction...", function_name);
    let tx_start = std::time::Instant::now();
    let exec_result = exec.execute_transaction(req).await;
    let tx_elapsed_ms = tx_start.elapsed().as_millis();
    
    // Release lock immediately after transaction is sent
    match lock_guard.release().await {
        Ok(_) => debug!("Lock released successfully"),
        Err(e) => {
            debug!("Warning: Failed to release lock cleanly: {}", e);
        }
    }
    
    let resp = match exec_result {
        Ok(r) => r,
        Err(e) => {
            debug!("Transaction execution error: {:?}", e);
            return Err(anyhow::anyhow!("Failed to execute transaction: {}", e));
        }
    };
    let tx_resp = resp.into_inner();
    
    // Check transaction was successful
    if tx_resp.finality.is_none() {
        return Err(anyhow::anyhow!("Transaction did not achieve finality"));
    }
    
    let tx_digest = tx_resp.transaction
        .as_ref()
        .and_then(|t| t.digest.as_ref())
        .context("Failed to get transaction digest")?
        .to_string();
    
    info!("[sui::registry] {} executed successfully with tx_digest: {} (took {}ms)", 
          function_name, tx_digest, tx_elapsed_ms);
    
    Ok(tx_digest)
}
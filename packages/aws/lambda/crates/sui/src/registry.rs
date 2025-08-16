use anyhow::{Context, Result};
use std::env;
use std::str::FromStr;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use sui_crypto::SuiSigner;
use tracing::{info, debug};
use db::KeyLock;
use crate::chain::{load_sender_from_env, get_reference_gas_price, pick_gas_object};

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
    // Move's String type is a struct { bytes: vector<u8> }
    // We need to serialize it properly for BCS encoding
    #[derive(serde::Serialize)]
    struct MoveString {
        bytes: Vec<u8>,
    }
    let move_string = MoveString {
        bytes: name.into_bytes(),
    };
    let name_arg = tb.input(sui_transaction_builder::Serialized(&move_string));
    
    // Function call: coordination::registry::create_registry(String)
    let func = sui_transaction_builder::Function::new(
        package_id,
        "registry".parse().unwrap(),
        "create_registry".parse().unwrap(),
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
    
    // Extract the registry ID from RegistryCreatedEvent, similar to how add.rs extracts SumEvent
    let registry_id = if let Some(executed) = tx_resp.transaction.as_ref() {
        if let Some(events) = executed.events.as_ref() {
            // Look for RegistryCreatedEvent
            events.events.iter()
                .find(|e| {
                    e.module.as_deref() == Some("registry") && 
                    e.event_type.as_deref().map(|t| t.ends_with("::RegistryCreatedEvent")).unwrap_or(false)
                })
                .and_then(|e| e.json.as_ref())
                .and_then(|json| extract_registry_id_from_json(json.as_ref()))
                .unwrap_or_else(|| {
                    debug!("No RegistryCreatedEvent found, trying to get from created objects");
                    // Fallback: try to get from effects
                    executed.effects.as_ref()
                        .and_then(|effects| {
                            effects.changed_objects
                                .iter()
                                .find(|obj| {
                                    // Check if it's a created object (input_state was DoesNotExist)
                                    obj.input_state == Some(proto::changed_object::InputObjectState::DoesNotExist as i32)
                                })
                                .and_then(|obj| obj.object_id.clone())
                        })
                        .unwrap_or_else(|| {
                            debug!("No created objects found, using tx digest as fallback");
                            tx_digest.clone()
                        })
                })
        } else {
            debug!("No events in transaction, using tx digest as fallback");
            tx_digest.clone()
        }
    } else {
        debug!("No transaction in response, using tx digest as fallback");
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
            m.get("id").and_then(|v| match &v.kind {
                Some(Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            })
        }
        _ => None,
    }
}

pub struct CreateRegistryResult {
    pub registry_id: String,
    pub tx_digest: String,
    pub admin_address: String,
}
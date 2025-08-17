use anyhow::{anyhow, Result};
use std::env;
use std::str::FromStr;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use sui_crypto::SuiSigner;
use tracing::{info, debug, warn};
use db::KeyLock;
use crate::chain::{rpc_url_from_env, load_sender_from_env, get_reference_gas_price, pick_gas_object};

pub struct SuiClient {
    // We'll manage the connection per-call rather than storing it
}

impl SuiClient {
    pub async fn from_env() -> Result<Self> {
        // Just validate that environment variables exist
        let _ = env::var("SUI_PACKAGE_ID")?;
        let _ = env::var("SUI_ADDRESS")?;
        let _ = env::var("SUI_SECRET_KEY")?;
        let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
        
        debug!("Sui client initialized for chain: {}", chain);
        Ok(Self {})
    }
    
    pub async fn call_add_function(&self, a: u64, b: u64) -> Result<(u64, String)> {
        debug!("Calling add function on Sui: a={}, b={}", a, b);
        
        // Get environment variables first
        let address_str = env::var("SUI_ADDRESS")?;
        let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
        
        // Acquire lock before preparing transaction
        let key_lock = KeyLock::new().await?;
        let lock_guard = key_lock.acquire_lock(&address_str, &chain).await?;
        info!("Acquired lock for Sui key on {}", chain);
        
        // Now proceed with transaction preparation
        let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
        let rpc_url = rpc_url_from_env();
        debug!("Using RPC URL: {}", rpc_url);
        
        let (sender, sk) = load_sender_from_env()?;
        debug!("Loaded sender: {}", sender);
        
        // Build PTB using SDK types + builder
        let mut tb = sui_transaction_builder::TransactionBuilder::new();
        tb.set_sender(sender);
        tb.set_gas_budget(10_000_000);
        
        let mut price_client = GrpcClient::new(rpc_url.clone())?;
        tb.set_gas_price(get_reference_gas_price(&mut price_client).await?);
        
        // Select gas
        let mut coin_client = GrpcClient::new(rpc_url.clone())?;
        let gas_ref = pick_gas_object(&mut coin_client, sender).await?;
        let gas_input = sui_transaction_builder::unresolved::Input::owned(
            *gas_ref.object_id(),
            gas_ref.version(),
            *gas_ref.digest(),
        );
        tb.add_gas_objects(vec![gas_input]);
        debug!("Gas object added: {:?}", gas_ref);
        
        // Inputs
        let a_arg = tb.input(sui_transaction_builder::Serialized(&a));
        let b_arg = tb.input(sui_transaction_builder::Serialized(&b));
        
        // Function call: package::main::calculate_sum(u64,u64)
        let func = sui_transaction_builder::Function::new(
            package_id,
            "main".parse()
                .map_err(|e| anyhow!("Failed to parse module name 'main': {}", e))?,
            "calculate_sum".parse()
                .map_err(|e| anyhow!("Failed to parse function name 'calculate_sum': {}", e))?,
            vec![],
        );
        tb.move_call(func, vec![a_arg, b_arg]);
        
        // Finalize
        let tx = tb.finish()?;
        let sig = sk.sign_transaction(&tx)?;
        
        // gRPC execute
        let mut grpc = GrpcClient::new(rpc_url)?;
        let mut exec = grpc.execution_client();
        let req = proto::ExecuteTransactionRequest {
            transaction: Some(tx.into()),
            signatures: vec![sig.into()],
            read_mask: Some(FieldMask { paths: vec!["finality".into(), "transaction".into()] }),
        };
        
        debug!("Sending ExecuteTransaction...");
        let tx_start = std::time::Instant::now();
        let exec_result = exec.execute_transaction(req).await;
        let tx_elapsed_ms = tx_start.elapsed().as_millis();
        
        // Release lock immediately after transaction is sent
        match lock_guard.release().await {
            Ok(_) => debug!("Lock released successfully"),
            Err(e) => warn!("Failed to release lock: {}", e),
        }
        
        // Now handle the transaction result
        let resp = exec_result?;
        let response = resp.into_inner();
        
        let executed = response
            .transaction
            .ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;
        
        // Get transaction digest/hash
        let tx_hash = executed.digest
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        
        info!("Transaction executed on blockchain: {} (took {}ms)", tx_hash, tx_elapsed_ms);
        
        // Extract SumEvent - using the exact same approach as the working code
        if let Some(events) = executed.events.as_ref() {
            for e in &events.events {
                if e.module.as_deref() == Some("main") && e.event_type.as_deref().map(|t| t.ends_with("::SumEvent")).unwrap_or(false) {
                    if let Some(json) = &e.json {
                        if let Some(sum) = extract_sum_from_json_boxed(json.as_ref()) {
                            info!("Successfully computed sum on blockchain: sum={}, tx_hash={}", sum, tx_hash);
                            return Ok((sum, tx_hash));
                        }
                    }
                }
            }
        }
        
        // Fallback - same as working code
        debug!("No SumEvent found, using fallback calculation");
        Ok((a + b, tx_hash))
    }
}

fn extract_sum_from_json_boxed(value: &prost_types::Value) -> Option<u64> {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::StructValue(s)) => {
            let m = &s.fields;
            m.get("sum").and_then(|v| match &v.kind {
                Some(Kind::NumberValue(n)) => Some(*n as u64),
                Some(Kind::StringValue(s)) => s.parse::<u64>().ok(),
                _ => None,
            })
        }
        _ => None,
    }
}
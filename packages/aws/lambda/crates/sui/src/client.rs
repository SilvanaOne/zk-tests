use anyhow::Result;
use std::env;
use std::str::FromStr;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use sui_crypto::SuiSigner;
use tracing::{info, debug, warn};
use db::KeyLock;

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
            "main".parse().unwrap(),
            "calculate_sum".parse().unwrap(),
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

fn rpc_url_from_env() -> String {
    let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
    match chain.as_str() {
        "mainnet" => "https://fullnode.mainnet.sui.io:443".to_string(),
        "testnet" => "https://fullnode.testnet.sui.io:443".to_string(),
        "devnet" => "https://fullnode.devnet.sui.io:443".to_string(),
        _ => "https://fullnode.devnet.sui.io:443".to_string(),
    }
}

fn load_sender_from_env() -> Result<(sui::Address, sui_crypto::ed25519::Ed25519PrivateKey)> {
    use base64ct::Encoding;
    let addr = sui::Address::from_str(&env::var("SUI_ADDRESS")?)?;
    let raw = env::var("SUI_SECRET_KEY")?;
    let key_part = raw.split_once(':').map(|(_, b)| b.to_string()).unwrap_or(raw);
    
    // Try bech32 "suiprivkey" first
    if key_part.starts_with("suiprivkey") {
        debug!("Decoding SUI_SECRET_KEY as bech32 suiprivkey");
        let (hrp, data, _variant) = bech32::decode(&key_part)?;
        if hrp != "suiprivkey" {
            return Err(anyhow::anyhow!("invalid bech32 hrp"));
        }
        let bytes: Vec<u8> = bech32::convert_bits(&data, 5, 8, false)?;
        if bytes.len() != 33 {
            return Err(anyhow::anyhow!("bech32 payload must be 33 bytes (flag || key)"));
        }
        if bytes[0] != 0x00 {
            return Err(anyhow::anyhow!("unsupported key scheme flag; only ed25519 supported"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[1..]);
        let sk = sui_crypto::ed25519::Ed25519PrivateKey::new(arr);
        return Ok((addr, sk));
    }
    
    // Else try base64 then hex
    let mut bytes = match base64ct::Base64::decode_vec(&key_part) {
        Ok(v) => v,
        Err(_) => {
            debug!("SUI_SECRET_KEY not base64; trying hex");
            if let Some(hex_str) = key_part.strip_prefix("0x") {
                hex::decode(hex_str)?
            } else {
                hex::decode(&key_part)?
            }
        }
    };
    
    if !bytes.is_empty() && (bytes[0] == 0x00 || bytes.len() == 33) {
        bytes = bytes[1..].to_vec();
    }
    
    if bytes.len() < 32 {
        return Err(anyhow::anyhow!("SUI_SECRET_KEY must contain at least 32 bytes"));
    }
    
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes[..32]);
    let sk = sui_crypto::ed25519::Ed25519PrivateKey::new(arr);
    Ok((addr, sk))
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

async fn get_reference_gas_price(client: &mut GrpcClient) -> Result<u64> {
    let mut ledger = client.ledger_client();
    let _resp = ledger
        .get_service_info(proto::GetServiceInfoRequest {})
        .await?
        .into_inner();
    // ServiceInfo does not expose gas price yet; default to 1000
    let price = 1_000u64;
    debug!("Using reference gas price: {}", price);
    Ok(price)
}

async fn pick_gas_object(client: &mut GrpcClient, sender: sui::Address) -> Result<sui::ObjectReference> {
    let mut live = client.live_data_client();
    debug!("Listing owned objects for sender: {}", sender);
    
    let resp = live
        .list_owned_objects(proto::ListOwnedObjectsRequest {
            owner: Some(sender.to_string()),
            page_size: Some(100),
            page_token: None,
            read_mask: Some(prost_types::FieldMask { 
                paths: vec![
                    "object_id".into(), 
                    "version".into(), 
                    "digest".into(), 
                    "object_type".into(),
                ]
            }),
            object_type: None,
        })
        .await?
        .into_inner();
    
    debug!("Owned objects returned: {}", resp.objects.len());
    
    let mut obj = resp
        .objects
        .into_iter()
        .find(|o| o.object_type.as_ref().map(|t| t.contains("::sui::SUI")).unwrap_or(true))
        .ok_or_else(|| anyhow::anyhow!("no owned objects to use as gas"))?;
    
    debug!(object = ?obj, "Selected object");
    
    if obj.digest.is_none() || obj.version.is_none() {
        debug!("Digest/version missing; fetching object details");
        let mut ledger = client.ledger_client();
        let object_id_str = obj.object_id.clone()
            .ok_or_else(|| anyhow::anyhow!("missing object id"))?;
        let got = ledger
            .get_object(proto::GetObjectRequest {
                object_id: Some(object_id_str.clone()),
                version: None,
                read_mask: None,
            })
            .await?
            .into_inner();
        
        if let Some(full) = got.object {
            debug!(object = ?full, "GetObject response");
            obj.object_id = full.object_id;
            obj.version = full.version;
            obj.digest = full.digest;
        }
    }
    
    let id = obj.object_id.as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing object id"))?
        .parse()?;
    let version = obj.version
        .ok_or_else(|| anyhow::anyhow!("missing version"))?;
    let digest = obj.digest.as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing digest"))?
        .parse()?;
    
    debug!("Gas coin chosen: id={}, version={}, digest={}", id, version, digest);
    Ok(sui::ObjectReference::new(id, version, digest))
}
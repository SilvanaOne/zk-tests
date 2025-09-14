use anyhow::Result;
use serde::Deserialize;
use std::env;
use std::str::FromStr;
use sui_crypto::{SuiSigner, Signer};
use sui_rpc::Client as GrpcClient;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_sdk_types as sui;

fn rpc_url_from_env() -> String {
    let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
    println!("[rpc] using chain={}", chain);
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
    let key_part = raw
        .split_once(':')
        .map(|(_, b)| b.to_string())
        .unwrap_or(raw);

    // Try bech32 "suiprivkey" first
    if key_part.starts_with("suiprivkey") {
        println!("[rpc] decoding SUI_SECRET_KEY as bech32 suiprivkey...");
        let (hrp, data, _variant) = bech32::decode(&key_part)?;
        if hrp != "suiprivkey" {
            return Err(anyhow::anyhow!("invalid bech32 hrp"));
        }
        let bytes: Vec<u8> = bech32::FromBase32::from_base32(&data)?;
        if bytes.len() != 33 {
            return Err(anyhow::anyhow!(
                "bech32 payload must be 33 bytes (flag || key)"
            ));
        }
        if bytes[0] != 0x00 {
            return Err(anyhow::anyhow!(
                "unsupported key scheme flag; only ed25519 supported"
            ));
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
            println!("[rpc] SUI_SECRET_KEY not base64; trying hex...");
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
        return Err(anyhow::anyhow!(
            "SUI_SECRET_KEY must contain at least 32 bytes of private key material"
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes[..32]);
    let sk = sui_crypto::ed25519::Ed25519PrivateKey::new(arr);
    Ok((addr, sk))
}

async fn get_reference_gas_price(client: &mut GrpcClient) -> Result<u64> {
    let mut ledger = client.ledger_client();
    let _resp = ledger
        .get_service_info(proto::GetServiceInfoRequest {})
        .await?
        .into_inner();
    // ServiceInfo does not expose gas price yet; default to 1000
    let price = 1_000u64;
    println!("[rpc] using reference gas price={}", price);
    Ok(price)
}

async fn pick_gas_object(
    client: &mut GrpcClient,
    sender: sui::Address,
) -> Result<sui::ObjectReference> {
    let mut live = client.live_data_client();
    println!("[rpc] listing owned objects for sender={}", sender);
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
                ],
            }),
            object_type: None,
        })
        .await?
        .into_inner();
    println!("[rpc] owned objects returned={}", resp.objects.len());
    let mut obj = resp
        .objects
        .into_iter()
        .find(|o| {
            o.object_type
                .as_ref()
                .map(|t| t.contains("::sui::SUI"))
                .unwrap_or(true)
        })
        .ok_or_else(|| anyhow::anyhow!("no owned objects to use as gas"))?;
    println!(
        "[rpc] selected object: id={:?} ver={:?} digest={:?} type={:?}",
        obj.object_id, obj.version, obj.digest, obj.object_type
    );
    if obj.digest.is_none() || obj.version.is_none() {
        println!("[rpc] digest/version missing; fetching object details via GetObject...");
        let mut ledger = client.ledger_client();
        let object_id_str = obj
            .object_id
            .clone()
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
            println!(
                "[rpc] GetObject: id={:?} ver={:?} digest={:?}",
                full.object_id, full.version, full.digest
            );
            obj.object_id = full.object_id;
            obj.version = full.version;
            obj.digest = full.digest;
        }
    }
    let id = obj
        .object_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing object id"))?
        .parse()?;
    let version = obj
        .version
        .ok_or_else(|| anyhow::anyhow!("missing version"))?;
    let digest = obj
        .digest
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing digest"))?
        .parse()?;
    println!(
        "[rpc] gas coin chosen: id={} ver={} digest={}",
        id, version, digest
    );
    Ok(sui::ObjectReference::new(id, version, digest))
}

fn extract_address_from_json_boxed(value: &prost_types::Value) -> Option<sui::Address> {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::StructValue(s)) => {
            let m = &s.fields;
            m.get("id").and_then(|v| match &v.kind {
                Some(Kind::StringValue(s)) => sui::Address::from_str(s).ok(),
                _ => None,
            })
        }
        _ => None,
    }
}

#[derive(Deserialize)]
struct StateCreatedEventJson {
    id: String,
    sum: Option<u64>,
}

#[derive(Deserialize)]
struct StateChangeEventJson {
    old_sum: u64,
    new_sum: u64,
}

fn try_decode_state_created_from_bcs(e: &proto::Event) -> Option<sui::Address> {
    let contents = e.contents.as_ref()?;
    let value_ref = contents.value.as_ref()?;
    let value_bytes: &[u8] = value_ref.as_ref();
    // Try parse as JSON first if available in BCS name context (rare), else raw BCS of address
    // Our Move event is struct { id: address, sum: u64 }
    // The BCS layout is address (32 bytes) followed by u64. We'll try a minimal parser.
    if value_bytes.len() >= 32 {
        let addr_bytes = &value_bytes[..32];
        if let Ok(addr) = sui::Address::from_bytes(addr_bytes) {
            return Some(addr);
        }
    }
    None
}

fn try_decode_state_change_from_bcs(e: &proto::Event) -> Option<(u64, u64)> {
    let contents = e.contents.as_ref()?;
    let value_ref = contents.value.as_ref()?;
    let value_bytes: &[u8] = value_ref.as_ref();
    // Layout: old_sum: u64 (LE) followed by new_sum: u64 (LE)
    if value_bytes.len() >= 16 {
        let (old_bytes, new_bytes) = value_bytes.split_at(8);
        let old_sum = u64::from_le_bytes(old_bytes.try_into().ok()?);
        let new_sum = u64::from_le_bytes(new_bytes[..8].try_into().ok()?);
        return Some((old_sum, new_sum));
    }
    None
}

fn extract_change_sums_from_json_boxed(value: &prost_types::Value) -> Option<(u64, u64)> {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::StructValue(s)) => {
            let m = &s.fields;
            let old_sum = m.get("old_sum").and_then(|v| match &v.kind {
                Some(Kind::NumberValue(n)) => Some(*n as u64),
                Some(Kind::StringValue(s)) => s.parse::<u64>().ok(),
                _ => None,
            });
            let new_sum = m.get("new_sum").and_then(|v| match &v.kind {
                Some(Kind::NumberValue(n)) => Some(*n as u64),
                Some(Kind::StringValue(s)) => s.parse::<u64>().ok(),
                _ => None,
            });
            match (old_sum, new_sum) {
                (Some(o), Some(n)) => Some((o, n)),
                _ => None,
            }
        }
        _ => None,
    }
}

pub async fn create_state() -> Result<sui::Address> {
    let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
    let rpc_url = rpc_url_from_env();
    println!("[rpc] rpc_url={}", rpc_url);
    let (sender, sk) = load_sender_from_env()?;
    println!("[rpc] sender={}", sender);

    // Build PTB
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
    println!(
        "[rpc] gas object_ref added: id={} ver={} digest={}",
        gas_ref.object_id(),
        gas_ref.version(),
        gas_ref.digest()
    );

    // Function call: package::main::create_state(&mut TxContext)
    let func = sui_transaction_builder::Function::new(
        package_id,
        "main".parse().unwrap(),
        "create_state".parse().unwrap(),
        vec![],
    );
    tb.move_call(func, vec![]);

    // Finalize
    let tx = tb.finish()?;
    let sig = sk.sign_transaction(&tx)?; // UserSignature

    // gRPC execute
    let mut grpc = GrpcClient::new(rpc_url)?;
    let mut exec = grpc.execution_client();
    let req = proto::ExecuteTransactionRequest {
        transaction: Some(tx.into()),
        signatures: vec![sig.into()],
        read_mask: Some(FieldMask {
            paths: vec!["finality".into(), "transaction".into()],
        }),
    };
    println!("[rpc] sending ExecuteTransaction (create_state)...");
    let resp = exec.execute_transaction(req).await?;
    let executed = resp
        .into_inner()
        .transaction
        .ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;

    // Extract StateCreatedEvent(id)
    if let Some(events) = executed.events.as_ref() {
        println!(
            "[rpc] create_state: events returned count={}",
            events.events.len()
        );
        for (idx, e) in events.events.iter().enumerate() {
            println!(
                "[rpc] event[{}]: type={:?} module={:?} json_present={}",
                idx,
                e.event_type,
                e.module,
                e.json.is_some()
            );
            println!("[rpc] event[{}] full={:#?}", idx, e);
            if e.event_type
                .as_deref()
                .map(|t| t.ends_with("::StateCreatedEvent"))
                .unwrap_or(false)
            {
                if let Some(json) = &e.json {
                    println!("[rpc] matched StateCreatedEvent json={:?}", json);
                    if let Some(addr) = extract_address_from_json_boxed(json.as_ref()) {
                        println!("[rpc] extracted state address={}", addr);
                        return Ok(addr);
                    } else {
                        println!("[rpc] failed to parse address from StateCreatedEvent json");
                    }
                }
                if let Some(addr) = try_decode_state_created_from_bcs(e) {
                    println!("[rpc] extracted state address from BCS={}", addr);
                    return Ok(addr);
                }
            }
        }
    } else {
        println!("[rpc] create_state: no events present in response");
    }

    Err(anyhow::anyhow!(
        "StateCreatedEvent not found in transaction events"
    ))
}

async fn fetch_initial_shared_version(rpc_url: &str, state_id: sui::Address) -> Result<u64> {
    let mut client = GrpcClient::new(rpc_url.to_string())?;
    let mut ledger = client.ledger_client();
    for attempt in 1..=60 {
        let resp = match ledger
            .get_object(proto::GetObjectRequest {
                object_id: Some(state_id.to_string()),
                version: None,
                read_mask: Some(prost_types::FieldMask {
                    paths: vec!["owner".into(), "object_id".into(), "version".into()],
                }),
            })
            .await
        {
            Ok(r) => r.into_inner(),
            Err(status) => {
                println!(
                    "[rpc] fetch_shared: attempt={} get_object error: {:?}",
                    attempt, status
                );
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                continue;
            }
        };
        if let Some(obj) = resp.object {
            println!(
                "[rpc] fetch_shared: attempt={} object_id={:?} version={:?} owner={:?}",
                attempt, obj.object_id, obj.version, obj.owner
            );
            if let Some(owner) = obj.owner {
                if let Some(kind_i32) = owner.kind {
                    if let Ok(kind_enum) = proto::owner::OwnerKind::try_from(kind_i32) {
                        if kind_enum == proto::owner::OwnerKind::Shared {
                            if let Some(v) = owner.version {
                                return Ok(v as u64);
                            } else {
                                println!(
                                    "[rpc] fetch_shared: shared owner but missing initial_shared_version; retrying..."
                                );
                            }
                        } else {
                            println!(
                                "[rpc] fetch_shared: owner kind={:?}; waiting for Shared...",
                                kind_enum
                            );
                        }
                    }
                }
            }
        } else {
            println!(
                "[rpc] fetch_shared: attempt={} object not found; retrying...",
                attempt
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    Err(anyhow::anyhow!(
        "object is not shared or missing initial_shared_version after retries"
    ))
}

pub async fn add_to_state(state_id: sui::Address, value: u64) -> Result<u64> {
    let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
    let rpc_url = rpc_url_from_env();
    println!("[rpc] rpc_url={}", rpc_url);
    let (sender, sk) = load_sender_from_env()?;
    println!("[rpc] sender={}", sender);

    // Get current epoch
    let mut grpc_epoch = GrpcClient::new(rpc_url.clone())?;
    let mut ledger = grpc_epoch.ledger_client();
    let service_info = ledger
        .get_service_info(proto::GetServiceInfoRequest {})
        .await?
        .into_inner();
    let epoch = service_info.epoch
        .ok_or_else(|| anyhow::anyhow!("failed to get current epoch"))?;
    println!("[rpc] current epoch={}", epoch);

    // Create signature for epoch || state_address
    let mut message = Vec::new();
    // Append epoch as 8 bytes (little-endian) using BCS encoding
    message.extend_from_slice(&bcs::to_bytes(&epoch)?);
    // Append state address as 32 bytes using BCS encoding
    message.extend_from_slice(&bcs::to_bytes(&state_id)?);

    // Sign the message with Ed25519 private key
    let signature: sui::Ed25519Signature = sk.try_sign(&message)?;
    let signature_bytes = signature.inner().to_vec();
    let public_key_bytes = sk.public_key().inner().to_vec();

    // Combine signature and public key: 64 bytes signature + 32 bytes public key
    let mut signature_with_pk = Vec::with_capacity(96);
    signature_with_pk.extend_from_slice(&signature_bytes);
    signature_with_pk.extend_from_slice(&public_key_bytes);
    println!("[rpc] signature with public key generated, len={}", signature_with_pk.len());

    let initial_shared_version = fetch_initial_shared_version(&rpc_url, state_id).await?;
    println!(
        "[rpc] state_id={} initial_shared_version={}",
        state_id, initial_shared_version
    );

    // Build PTB
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
    println!(
        "[rpc] gas object_ref added: id={} ver={} digest={}",
        gas_ref.object_id(),
        gas_ref.version(),
        gas_ref.digest()
    );

    // Inputs
    let state_arg =
        sui_transaction_builder::unresolved::Input::shared(state_id, initial_shared_version, true);
    let state_arg = tb.input(state_arg);
    let value_arg = tb.input(sui_transaction_builder::Serialized(&value));
    let signature_arg = tb.input(sui_transaction_builder::Serialized(&signature_with_pk));

    // Function call: package::main::add_to_state(&mut State, u64, vector<u8>)
    let func = sui_transaction_builder::Function::new(
        package_id,
        "main".parse().unwrap(),
        "add_to_state".parse().unwrap(),
        vec![],
    );
    tb.move_call(func, vec![state_arg, value_arg, signature_arg]);

    // Finalize
    let tx = tb.finish()?;
    let sig = sk.sign_transaction(&tx)?; // UserSignature

    // gRPC execute
    let mut grpc = GrpcClient::new(rpc_url)?;
    let mut exec = grpc.execution_client();
    let req = proto::ExecuteTransactionRequest {
        transaction: Some(tx.into()),
        signatures: vec![sig.into()],
        read_mask: Some(FieldMask {
            paths: vec!["finality".into(), "transaction".into()],
        }),
    };
    println!("[rpc] sending ExecuteTransaction (add_to_state)...");
    let resp = exec.execute_transaction(req).await?;
    let executed = resp
        .into_inner()
        .transaction
        .ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;

    // Extract StateChangeEvent(new_sum)
    if let Some(events) = executed.events.as_ref() {
        println!(
            "[rpc] add_to_state: events returned count={}",
            events.events.len()
        );
        for (idx, e) in events.events.iter().enumerate() {
            println!(
                "[rpc] event[{}]: type={:?} module={:?} json_present={}",
                idx,
                e.event_type,
                e.module,
                e.json.is_some()
            );
            println!("[rpc] event[{}] full={:#?}", idx, e);
            if e.event_type
                .as_deref()
                .map(|t| t.ends_with("::StateChangeEvent"))
                .unwrap_or(false)
            {
                if let Some(json) = &e.json {
                    println!("[rpc] matched StateChangeEvent json={:?}", json);
                    if let Some((_old, new)) = extract_change_sums_from_json_boxed(json.as_ref()) {
                        println!("[rpc] extracted new_sum={}", new);
                        return Ok(new);
                    } else {
                        println!("[rpc] failed to parse new_sum from StateChangeEvent json");
                    }
                }
                if let Some((_old, new)) = try_decode_state_change_from_bcs(e) {
                    println!("[rpc] extracted new_sum from BCS={}", new);
                    return Ok(new);
                }
            }
        }
    } else {
        println!("[rpc] add_to_state: no events present in response");
    }

    Err(anyhow::anyhow!(
        "StateChangeEvent not found in transaction events"
    ))
}

pub async fn multiple_add_to_state(state_id: sui::Address, values: Vec<u64>) -> Result<Vec<u64>> {
    if values.is_empty() {
        return Ok(vec![]);
    }

    let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
    let rpc_url = rpc_url_from_env();
    println!("[rpc] rpc_url={}", rpc_url);
    let (sender, sk) = load_sender_from_env()?;
    println!("[rpc] sender={}", sender);

    // Get current epoch
    let mut grpc_epoch = GrpcClient::new(rpc_url.clone())?;
    let mut ledger = grpc_epoch.ledger_client();
    let service_info = ledger
        .get_service_info(proto::GetServiceInfoRequest {})
        .await?
        .into_inner();
    let epoch = service_info.epoch
        .ok_or_else(|| anyhow::anyhow!("failed to get current epoch"))?;
    println!("[rpc] current epoch={}", epoch);

    // Create signature for epoch || state_address
    let mut message = Vec::new();
    // Append epoch as 8 bytes (little-endian) using BCS encoding
    message.extend_from_slice(&bcs::to_bytes(&epoch)?);
    // Append state address as 32 bytes using BCS encoding
    message.extend_from_slice(&bcs::to_bytes(&state_id)?);

    // Sign the message with Ed25519 private key
    let signature: sui::Ed25519Signature = sk.try_sign(&message)?;
    let signature_bytes = signature.inner().to_vec();
    let public_key_bytes = sk.public_key().inner().to_vec();

    // Combine signature and public key: 64 bytes signature + 32 bytes public key
    let mut signature_with_pk = Vec::with_capacity(96);
    signature_with_pk.extend_from_slice(&signature_bytes);
    signature_with_pk.extend_from_slice(&public_key_bytes);
    println!("[rpc] signature with public key generated, len={}", signature_with_pk.len());

    let initial_shared_version = fetch_initial_shared_version(&rpc_url, state_id).await?;
    println!(
        "[rpc] state_id={} initial_shared_version={}",
        state_id, initial_shared_version
    );

    // Build PTB
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
    println!(
        "[rpc] gas object_ref added: id={} ver={} digest={}",
        gas_ref.object_id(),
        gas_ref.version(),
        gas_ref.digest()
    );

    // Create shared state input (mutable)
    let state_input = sui_transaction_builder::unresolved::Input::shared(state_id, initial_shared_version, true);
    let state_arg = tb.input(state_input);

    // Chain multiple add_to_state calls
    // Each call modifies the shared state and returns the new sum
    let mut results = Vec::new();
    for (i, value) in values.iter().enumerate() {
        println!("[rpc] adding move call #{}: add_to_state(&mut State, {})", i, value);

        let value_arg = tb.input(sui_transaction_builder::Serialized(value));
        let signature_arg = tb.input(sui_transaction_builder::Serialized(&signature_with_pk));

        // Function call: package::main::add_to_state(&mut State, u64, vector<u8>)
        let func = sui_transaction_builder::Function::new(
            package_id,
            "main".parse().unwrap(),
            "add_to_state".parse().unwrap(),
            vec![],
        );

        // add_to_state takes the shared state (which is automatically passed by reference),
        // the value to add, and signature with embedded public key
        let result = tb.move_call(func, vec![state_arg.clone(), value_arg, signature_arg]);
        results.push(result);
    }

    // Finalize
    let tx = tb.finish()?;
    let sig = sk.sign_transaction(&tx)?;

    // gRPC execute
    let mut grpc = GrpcClient::new(rpc_url)?;
    let mut exec = grpc.execution_client();
    let req = proto::ExecuteTransactionRequest {
        transaction: Some(tx.into()),
        signatures: vec![sig.into()],
        read_mask: Some(FieldMask {
            paths: vec![
                "finality".into(), 
                "transaction".into(), 
                "transaction.events".into(),
                "transaction.events.events".into(),
                "transaction.events.events.contents".into(),
            ],
        }),
    };
    println!("[rpc] sending ExecuteTransaction with {} chained add_to_state calls...", values.len());
    let resp = exec.execute_transaction(req).await?;
    let executed = resp
        .into_inner()
        .transaction
        .ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;

    // Extract all StateChangeEvents
    let mut sums = Vec::new();
    if let Some(events) = executed.events.as_ref() {
        for e in &events.events {
            if e.event_type
                .as_deref()
                .map(|t| t.ends_with("::StateChangeEvent"))
                .unwrap_or(false)
            {
                if let Some(contents) = &e.contents {
                    if let Some(value) = &contents.value {
                        // StateChangeEvent has 2 u64 fields: old_sum, new_sum (16 bytes total)
                        // We want the new_sum which is at bytes 8-16
                        if value.len() >= 16 {
                            let new_sum = u64::from_le_bytes(value[8..16].try_into().unwrap());
                            sums.push(new_sum);
                        }
                    }
                }
            }
        }
    }

    if sums.len() != values.len() {
        return Err(anyhow::anyhow!("Expected {} StateChangeEvents but got {}", values.len(), sums.len()));
    }

    Ok(sums)
}

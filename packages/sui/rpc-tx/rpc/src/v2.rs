use anyhow::Result;
use std::env;
use std::str::FromStr;
use sui_crypto::SuiSigner;
use sui_rpc::client::v2::Client as GrpcClient;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2 as proto;
use sui_sdk_types as sui;

fn rpc_url_from_env() -> String {
    let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
    println!("[rpc-v2] using chain={}", chain);
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
        println!("[rpc-v2] decoding SUI_SECRET_KEY as bech32 suiprivkey...");
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
            println!("[rpc-v2] SUI_SECRET_KEY not base64; trying hex...");
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
        .get_service_info(proto::GetServiceInfoRequest::default())
        .await?
        .into_inner();
    // ServiceInfo does not expose gas price yet; default to 1000
    let price = 1_000u64;
    println!("[rpc-v2] using reference gas price={}", price);
    Ok(price)
}

async fn pick_gas_object(
    client: &mut GrpcClient,
    sender: sui::Address,
) -> Result<sui::ObjectReference> {
    let mut state = client.state_client();
    println!("[rpc-v2] listing owned objects for sender={}", sender);
    let mut req = proto::ListOwnedObjectsRequest::default();
    req.owner = Some(sender.to_string());
    req.page_size = Some(100);
    req.page_token = None;
    req.read_mask = Some(prost_types::FieldMask {
        paths: vec![
            "object_id".into(),
            "version".into(),
            "digest".into(),
            "object_type".into(),
        ],
    });
    req.object_type = None;
    let resp = state
        .list_owned_objects(req)
        .await?
        .into_inner();
    println!("[rpc-v2] owned objects returned={}", resp.objects.len());
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
        "[rpc-v2] selected object: id={:?} ver={:?} digest={:?} type={:?}",
        obj.object_id, obj.version, obj.digest, obj.object_type
    );
    if obj.digest.is_none() || obj.version.is_none() {
        println!("[rpc-v2] digest/version missing; fetching object details via GetObject...");
        let mut ledger = client.ledger_client();
        let object_id_str = obj
            .object_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing object id"))?;
        let mut get_req = proto::GetObjectRequest::default();
        get_req.object_id = Some(object_id_str.clone());
        get_req.version = None;
        get_req.read_mask = None;
        let got = ledger
            .get_object(get_req)
            .await?
            .into_inner();
        if let Some(full) = got.object {
            println!(
                "[rpc-v2] GetObject: id={:?} ver={:?} digest={:?}",
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
        "[rpc-v2] gas coin chosen: id={} ver={} digest={}",
        id, version, digest
    );
    Ok(sui::ObjectReference::new(id, version, digest))
}

fn extract_address_from_json_boxed(value: &prost_types::Value) -> Option<sui::Address> {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::StructValue(s)) => {
            for (key, val) in &s.fields {
                if key == "id" {
                    if let Some(Kind::StringValue(addr_str)) = val.kind.as_ref() {
                        if let Ok(addr) = sui::Address::from_str(addr_str) {
                            return Some(addr);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    None
}

pub async fn create_state() -> Result<sui::Address> {
    let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
    let rpc_url = rpc_url_from_env();
    println!("[rpc-v2] rpc_url={}", rpc_url);
    let (sender, sk) = load_sender_from_env()?;
    println!("[rpc-v2] sender={}", sender);

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

    // Call create_state
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
    let mut req = proto::ExecuteTransactionRequest::default();
    req.transaction = Some(tx.into());
    req.signatures = vec![sig.into()];
    req.read_mask = Some(FieldMask {
        paths: vec!["finality".into(), "transaction".into()],
    });
    println!("[rpc-v2] sending ExecuteTransaction (create_state)...");
    let resp = exec.execute_transaction(req).await?;
    let executed = resp
        .into_inner()
        .transaction
        .ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;

    // Extract StateCreatedEvent(id)
    if let Some(events) = executed.events.as_ref() {
        println!(
            "[rpc-v2] create_state: events returned count={}",
            events.events.len()
        );
        for (idx, e) in events.events.iter().enumerate() {
            println!(
                "[rpc-v2] event[{}]: type={:?} module={:?} json_present={}",
                idx,
                e.event_type,
                e.module,
                e.json.is_some()
            );
            println!("[rpc-v2] event[{}] full={:#?}", idx, e);
            if e.event_type
                .as_deref()
                .map(|t| t.ends_with("::StateCreatedEvent"))
                .unwrap_or(false)
            {
                if let Some(json) = &e.json {
                    println!("[rpc-v2] matched StateCreatedEvent json={:?}", json);
                    if let Some(addr) = extract_address_from_json_boxed(json.as_ref()) {
                        println!("[rpc-v2] extracted state address={}", addr);
                        return Ok(addr);
                    } else {
                        println!("[rpc-v2] failed to parse address from StateCreatedEvent json");
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!("StateCreatedEvent not found in transaction events"))
}
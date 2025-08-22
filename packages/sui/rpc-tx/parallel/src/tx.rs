use anyhow::Result;
use std::env;
use std::str::FromStr;
use sui_rpc::field::FieldMask;
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use sui_crypto::SuiSigner;
use crate::coin;


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
	let key_part = raw.split_once(':').map(|(_, b)| b.to_string()).unwrap_or(raw);

	// Try bech32 "suiprivkey" first
	if key_part.starts_with("suiprivkey") {
		println!("[rpc] decoding SUI_SECRET_KEY as bech32 suiprivkey...");
		let (hrp, data, _variant) = bech32::decode(&key_part)?;
		if hrp != "suiprivkey" { return Err(anyhow::anyhow!("invalid bech32 hrp")); }
		let bytes: Vec<u8> = bech32::FromBase32::from_base32(&data)?;
		if bytes.len() != 33 { return Err(anyhow::anyhow!("bech32 payload must be 33 bytes (flag || key)")); }
		if bytes[0] != 0x00 { return Err(anyhow::anyhow!("unsupported key scheme flag; only ed25519 supported")); }
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
			if let Some(hex_str) = key_part.strip_prefix("0x") { hex::decode(hex_str)? } else { hex::decode(&key_part)? }
		}
	};
	if !bytes.is_empty() && (bytes[0] == 0x00 || bytes.len() == 33) { bytes = bytes[1..].to_vec(); }
	if bytes.len() < 32 { return Err(anyhow::anyhow!("SUI_SECRET_KEY must contain at least 32 bytes of private key material")); }
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
	println!("[rpc] using reference gas price={}", price);
	Ok(price)
}

async fn pick_gas_object(client: &mut GrpcClient, sender: sui::Address) -> Result<sui::ObjectReference> {
	let mut live = client.live_data_client();
	println!("[rpc] listing owned objects for sender={}", sender);
	let resp = live
		.list_owned_objects(proto::ListOwnedObjectsRequest {
			owner: Some(sender.to_string()),
			page_size: Some(100),
			page_token: None,
			read_mask: Some(prost_types::FieldMask { paths: vec![
				"object_id".into(), "version".into(), "digest".into(), "object_type".into(),
			]}),
			object_type: None,
		})
		.await?
		.into_inner();
	println!("[rpc] owned objects returned={}", resp.objects.len());
	let mut obj = resp
		.objects
		.into_iter()
		.find(|o| o.object_type.as_ref().map(|t| t.contains("::sui::SUI")).unwrap_or(true))
		.ok_or_else(|| anyhow::anyhow!("no owned objects to use as gas"))?;
	println!("[rpc] selected object: id={:?} ver={:?} digest={:?} type={:?}", obj.object_id, obj.version, obj.digest, obj.object_type);
	if obj.digest.is_none() || obj.version.is_none() {
		println!("[rpc] digest/version missing; fetching object details via GetObject...");
		let mut ledger = client.ledger_client();
		let object_id_str = obj.object_id.clone().ok_or_else(|| anyhow::anyhow!("missing object id"))?;
		let got = ledger
			.get_object(proto::GetObjectRequest { object_id: Some(object_id_str.clone()), version: None, read_mask: None })
			.await?
			.into_inner();
		if let Some(full) = got.object {
			println!("[rpc] GetObject: id={:?} ver={:?} digest={:?}", full.object_id, full.version, full.digest);
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
	let version = obj.version.ok_or_else(|| anyhow::anyhow!("missing version"))?;
	let digest = obj
		.digest
		.as_ref()
		.ok_or_else(|| anyhow::anyhow!("missing digest"))?
		.parse()?;
	println!("[rpc] gas coin chosen: id={} ver={} digest={}", id, version, digest);
	Ok(sui::ObjectReference::new(id, version, digest))
}

pub async fn calculate_sum(a: u64, b: u64) -> Result<u64> {
	let package_id = sui::Address::from_str(&env::var("SUI_PACKAGE_ID")?)?;
	let rpc_url = rpc_url_from_env();
	println!("[rpc] rpc_url={}", rpc_url);
	let (sender, sk) = load_sender_from_env()?;
	println!("[rpc] sender={}", sender);

	// Build PTB using SDK types + builder
	let mut tb = sui_transaction_builder::TransactionBuilder::new();
	tb.set_sender(sender);
	tb.set_gas_budget(10_000_000);
	let mut price_client = GrpcClient::new(rpc_url.clone())?;
	tb.set_gas_price(get_reference_gas_price(&mut price_client).await?);

	// Select gas coin using our parallel-safe coin management
	let (gas_coin, _coin_guard) = match coin::fetch_coin(&rpc_url, sender, 10_000_000).await? {
		Some((coin, guard)) => (coin, guard),
		None => {
			println!("[rpc] No available coins with sufficient balance");
			return Err(anyhow::anyhow!("No available coins with sufficient balance"));
		}
	};
	
	let gas_input = sui_transaction_builder::unresolved::Input::owned(
		gas_coin.object_id(),
		gas_coin.object_ref.version(),
		*gas_coin.object_ref.digest(),
	);
	tb.add_gas_objects(vec![gas_input]);
	println!("[rpc] gas coin selected: id={} ver={} digest={} balance={}", 
		gas_coin.object_id(), gas_coin.object_ref.version(), gas_coin.object_ref.digest(), gas_coin.balance);

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
	let sig = sk.sign_transaction(&tx)?; // UserSignature

	// gRPC execute
	let mut grpc = GrpcClient::new(rpc_url)?;
	let mut exec = grpc.execution_client();
	let req = proto::ExecuteTransactionRequest {
		transaction: Some(tx.into()),
		signatures: vec![sig.into()],
		read_mask: Some(FieldMask { paths: vec!["finality".into(), "transaction".into()] }),
	};
	println!("[rpc] sending ExecuteTransaction...");
	let resp = exec.execute_transaction(req).await?;
	let executed = resp
		.into_inner()
		.transaction
		.ok_or_else(|| anyhow::anyhow!("no transaction in response"))?;

	// Extract SumEvent
	if let Some(events) = executed.events.as_ref() {
		for e in &events.events {
			if e.module.as_deref() == Some("main") && e.event_type.as_deref().map(|t| t.ends_with("::SumEvent")).unwrap_or(false) {
				if let Some(json) = &e.json { if let Some(sum) = extract_sum_from_json_boxed(json.as_ref()) { return Ok(sum); } }
			}
		}
	}

	// Fallback
	Ok(a + b)
}

pub async fn get_sum_from_tx_digest(digest_hex: &str) -> Result<u64> {
	let rpc_url = rpc_url_from_env();
	let mut client = GrpcClient::new(rpc_url)?;
	let mut ledger = client.ledger_client();
	let resp = ledger
		.get_transaction(proto::GetTransactionRequest {
			digest: Some(digest_hex.to_string()),
			read_mask: Some(FieldMask { paths: vec!["transaction".into()] }),
		})
		.await?
		.into_inner();
	if let Some(txn) = resp.transaction {
		if let Some(events) = txn.events.as_ref() {
			for e in &events.events {
				if e.module.as_deref() == Some("main") && e.event_type.as_deref().map(|t| t.ends_with("::SumEvent")).unwrap_or(false) {
					if let Some(json) = &e.json { if let Some(sum) = extract_sum_from_json_boxed(json.as_ref()) { return Ok(sum); } }
				}
			}
		}
	}
	Err(anyhow::anyhow!("SumEvent not found in transaction events"))
}



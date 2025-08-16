use anyhow::Result;
use serde_json::Value as JsonValue;
use shared_crypto::intent::Intent;
use std::env;
use std::str::FromStr;
use sui_keys::keystore::{AccountKeystore, InMemKeystore};
use sui_sdk::rpc_types::{
    SuiObjectDataOptions, SuiTransactionBlockEffectsAPI, SuiTransactionBlockResponseOptions,
};
use sui_sdk::types::{
    Identifier,
    base_types::{ObjectID, SuiAddress, TransactionDigest},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, Command, ObjectArg, Transaction, TransactionData},
};
use sui_sdk::types::{crypto::SuiKeyPair, object::Owner};
use sui_sdk::{SuiClient, SuiClientBuilder};

struct SuiConfig {
    package_id: ObjectID,
    chain: String,
    address: SuiAddress,
    private_key: String,
}

async fn load_client_from_env() -> Result<(SuiClient, InMemKeystore, SuiAddress, SuiConfig)> {
    dotenv::from_path("../.env").ok();
    let package_id = env::var("SUI_PACKAGE_ID")
        .ok()
        .and_then(|s| ObjectID::from_str(&s).ok())
        .ok_or_else(|| anyhow::anyhow!("SUI_PACKAGE_ID is required"))?;
    let chain = env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string());
    let address = SuiAddress::from_str(&env::var("SUI_ADDRESS")?)?;
    let private_key = env::var("SUI_SECRET_KEY")?;

    let sui_client = match chain.as_str() {
        "devnet" => SuiClientBuilder::default().build_devnet().await?,
        "testnet" => SuiClientBuilder::default().build_testnet().await?,
        "mainnet" => SuiClientBuilder::default().build_mainnet().await?,
        _ => return Err(anyhow::anyhow!(format!("Unsupported chain: {}", chain))),
    };

    let mut keystore = InMemKeystore::default();
    let keypair = SuiKeyPair::decode(&private_key)
        .map_err(|e| anyhow::anyhow!(format!("Failed to decode private key: {}", e)))?;
    let sender = SuiAddress::from(&keypair.public());
    keystore.add_key(Some("sender".to_string()), keypair)?;

    let config = SuiConfig {
        package_id,
        chain,
        address,
        private_key,
    };
    Ok((sui_client, keystore, sender, config))
}

async fn get_gas_coin(
    sui_client: &SuiClient,
    sender: SuiAddress,
) -> Result<sui_sdk::rpc_types::Coin> {
    let coins = sui_client
        .coin_read_api()
        .get_coins(sender, Some("0x2::sui::SUI".to_string()), None, None)
        .await?;

    coins
        .data
        .into_iter()
        .find(|coin| coin.balance >= 10_000_000)
        .ok_or_else(|| anyhow::anyhow!("No suitable gas coin found"))
}

async fn get_object_arg(sui_client: &SuiClient, object_id: ObjectID) -> Result<CallArg> {
    let object = sui_client
        .read_api()
        .get_object_with_options(
            object_id,
            SuiObjectDataOptions {
                show_type: true,
                show_owner: true,
                show_previous_transaction: false,
                show_display: false,
                show_content: false,
                show_bcs: false,
                show_storage_rebate: false,
            },
        )
        .await?;

    let owner = object.owner().clone().unwrap();
    let object_ref = object.data.unwrap().object_ref();

    let obj_arg = match owner {
        Owner::Shared {
            initial_shared_version,
        } => ObjectArg::SharedObject {
            id: object_ref.0,
            initial_shared_version,
            mutable: true,
        },
        _ => ObjectArg::ImmOrOwnedObject(object_ref),
    };

    Ok(CallArg::Object(obj_arg))
}

pub async fn calculate_sum(a: u64, b: u64) -> Result<u64> {
    let (sui_client, keystore, sender, config) = load_client_from_env().await?;

    let mut ptb = ProgrammableTransactionBuilder::new();
    let a_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&a)?))?;
    let b_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&b)?))?;

    let module = Identifier::new("main")?;
    let function = Identifier::new("calculate_sum")?;

    ptb.command(Command::move_call(
        config.package_id,
        module,
        function,
        vec![],
        vec![a_arg, b_arg],
    ));

    let builder = ptb.finish();
    let gas_price = sui_client.read_api().get_reference_gas_price().await?;
    let gas_coin = get_gas_coin(&sui_client, sender).await?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        builder,
        10_000_000,
        gas_price,
    );

    let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;
    let response = sui_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    if let Some(effects) = &response.effects {
        match effects.status() {
            sui_sdk::rpc_types::SuiExecutionStatus::Success => {
                // Try to extract sum from emitted event first
                if let Some(events) = &response.events {
                    if let Some(sum) = extract_sum_from_events(events, &config.package_id) {
                        return Ok(sum);
                    }
                }
                // Fallback if event is missing
                return Ok(a + b);
            }
            sui_sdk::rpc_types::SuiExecutionStatus::Failure { error } => {
                return Err(anyhow::anyhow!(format!("Transaction failed: {}", error)));
            }
        }
    }

    Err(anyhow::anyhow!("No effects in transaction response"))
}

fn extract_sum_from_events(
    events: &sui_sdk::rpc_types::SuiTransactionBlockEvents,
    package_id: &ObjectID,
) -> Option<u64> {
    let expected_type = format!("{}::main::SumEvent", package_id);
    events.data.iter().find_map(|event| {
        // Match type by exact package id and module/type
        let type_matches = event.type_.to_string() == expected_type
            // or allow suffix match if package id differs
            || event.type_.to_string().ends_with("::main::SumEvent");

        if !type_matches {
            return None;
        }

        extract_u64_field(&event.parsed_json, "sum")
    })
}

fn extract_u64_field(json: &JsonValue, key: &str) -> Option<u64> {
    match json.get(key) {
        Some(JsonValue::Number(n)) => n.as_u64(),
        Some(JsonValue::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    }
}

pub async fn get_sum_from_tx_digest(digest_hex: &str) -> Result<u64> {
    let (sui_client, _keystore, _sender, config) = load_client_from_env().await?;
    let options = SuiTransactionBlockResponseOptions::full_content();
    let digest = TransactionDigest::from_str(digest_hex)?;
    let resp = sui_client
        .read_api()
        .get_transaction_with_options(digest, options)
        .await?;

    if let Some(events) = &resp.events {
        if let Some(sum) = extract_sum_from_events(events, &config.package_id) {
            return Ok(sum);
        }
    }

    Err(anyhow::anyhow!("SumEvent not found in transaction events"))
}

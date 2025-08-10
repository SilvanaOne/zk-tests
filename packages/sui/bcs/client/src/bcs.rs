use futures::stream::StreamExt;
use move_core_types::u256::U256;
use serde::{Deserialize, Serialize};
use shared_crypto::intent::Intent;
use std::sync::OnceLock;
use sui_keys::keystore::{AccountKeystore, InMemKeystore};
use sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI;
use sui_sdk::rpc_types::{Coin, SuiObjectDataOptions, SuiTransactionBlockResponseOptions};
use sui_sdk::types::{
    Identifier,
    base_types::ObjectID,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, Command, ObjectArg, Transaction, TransactionData},
};
use sui_sdk::types::{base_types::SuiAddress, crypto::SuiKeyPair, object::Owner};
use sui_sdk::{SuiClient, SuiClientBuilder};

static SUI_CLIENT: OnceLock<SuiClient> = OnceLock::new();
const PACKAGE_ID: &str = "0xb422b7ae1d4cebb348a88c2a47328ae2153c81ae1cad8e571d7bde35e68984ff";
const GAS_BUDGET: u64 = 100_000_000; // 0.1 SUI

/// UserRequest struct that mirrors the Move struct
/// This needs to match exactly the field order and types in the Move struct
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserRequest {
    pub name: String,
    pub data: U256,
    pub signature: Vec<u8>,
}

pub async fn get_sui_client() -> Result<&'static SuiClient, anyhow::Error> {
    if let Some(client) = SUI_CLIENT.get() {
        Ok(client)
    } else {
        let client = SuiClientBuilder::default().build_devnet().await?;
        let _ = SUI_CLIENT.set(client.clone());
        let client = SUI_CLIENT.get();
        match client {
            Some(client) => Ok(client),
            None => Err(anyhow::anyhow!("Sui client not found")),
        }
    }
}

fn get_keypair_from_env() -> Result<SuiKeyPair, anyhow::Error> {
    let private_key = std::env::var("SUI_KEY")
        .map_err(|_| anyhow::anyhow!("SUI_KEY environment variable not set"))?;

    SuiKeyPair::decode(&private_key)
        .map_err(|e| anyhow::anyhow!("Failed to decode private key: {}", e))
}

async fn get_gas_coin(
    sui_client: &SuiClient,
    sender: &SuiAddress,
    min_balance: u64,
) -> Result<Coin, anyhow::Error> {
    let coin_type = "0x2::sui::SUI".to_string();
    let mut coins_stream = sui_client
        .coin_read_api()
        .get_coins_stream(*sender, Some(coin_type))
        .boxed();

    while let Some(coin) = coins_stream.next().await {
        if coin.balance >= min_balance {
            return Ok(coin);
        }
    }

    Err(anyhow::anyhow!("No coin found with sufficient balance"))
}

/// Create a new user
/// Returns the shared object id of the user as hex string 0x...
pub async fn create_user(
    name: String,
    data: U256,
    signature: Vec<u8>,
) -> Result<String, anyhow::Error> {
    let client = get_sui_client().await?;
    let keypair = get_keypair_from_env()?;
    let sender = SuiAddress::from(&keypair.public());

    let mut keystore = InMemKeystore::default();
    keystore.add_key(Some("sender".to_string()), keypair)?;

    let mut ptb = ProgrammableTransactionBuilder::new();

    // Prepare arguments for create_user(name: String, data: u256, signature: vector<u8>, ctx: &mut TxContext)
    let name_input = CallArg::Pure(bcs::to_bytes(&name)?);
    let name_arg = ptb.input(name_input)?;

    let data_input = CallArg::Pure(bcs::to_bytes(&data)?);
    let data_arg = ptb.input(data_input)?;

    let signature_input = CallArg::Pure(bcs::to_bytes(&signature)?);
    let signature_arg = ptb.input(signature_input)?;

    let package_id = ObjectID::from_hex_literal(PACKAGE_ID)?;
    let module = Identifier::new("main")?;
    let function = Identifier::new("create_user")?;

    ptb.command(Command::move_call(
        package_id,
        module,
        function,
        vec![], // no type arguments
        vec![name_arg, data_arg, signature_arg],
    ));

    let builder = ptb.finish();
    let gas_price = client.read_api().get_reference_gas_price().await?;
    let gas_coin = get_gas_coin(client, &sender, GAS_BUDGET).await?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        builder,
        GAS_BUDGET,
        gas_price,
    );

    // Sign transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

    // Execute transaction
    let result = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", result.digest);

    // Print transaction errors if any
    if let Some(effects) = &result.effects {
        if effects.status().is_err() {
            println!("❌ Transaction failed!");
            println!("   Status: {:?}", effects.status());
        } else {
            println!("✅ Transaction succeeded");
        }
    }

    // Find the created shared object ID from the transaction effects
    if let Some(effects) = result.effects {
        for created in effects.created() {
            let object_ref = created.reference.to_object_ref();
            let object_info = client
                .read_api()
                .get_object_with_options(object_ref.0, SuiObjectDataOptions::new().with_owner())
                .await?;

            if let Some(data) = object_info.data {
                if let Some(Owner::Shared { .. }) = data.owner {
                    return Ok(format!("0x{}", object_ref.0));
                }
            }
        }
    }

    Err(anyhow::anyhow!("Failed to find created shared object"))
}

/// Make a request to the user  
/// Returns the tx digest and the UserStateEvent data
pub async fn user_request_1(
    object_id: String,
    name: String,
    data: U256,
    signature: Vec<u8>,
) -> Result<(String, Option<serde_json::Value>), anyhow::Error> {
    let client = get_sui_client().await?;
    let keypair = get_keypair_from_env()?;
    let sender = SuiAddress::from(&keypair.public());

    let mut keystore = InMemKeystore::default();
    keystore.add_key(Some("sender".to_string()), keypair)?;

    let mut ptb = ProgrammableTransactionBuilder::new();

    // Get the shared UserState object
    let user_state_id = ObjectID::from_hex_literal(&object_id)?;
    let object_info = client
        .read_api()
        .get_object_with_options(
            user_state_id,
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

    let owner = object_info
        .owner()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Object owner not found"))?;
    let object_ref = object_info
        .data
        .ok_or_else(|| anyhow::anyhow!("Object data not found"))?
        .object_ref();

    let obj_arg = match owner {
        Owner::Shared {
            initial_shared_version,
        } => ObjectArg::SharedObject {
            id: object_ref.0,
            initial_shared_version,
            mutable: true,
        },
        _ => return Err(anyhow::anyhow!("Expected shared object")),
    };

    let user_state_arg = ptb.input(CallArg::Object(obj_arg))?;

    // Prepare other arguments for user_request_1(state: &mut UserState, name: String, data: u256, signature: vector<u8>, _ctx: &mut TxContext)
    let name_input = CallArg::Pure(bcs::to_bytes(&name)?);
    let name_arg = ptb.input(name_input)?;

    let data_input = CallArg::Pure(bcs::to_bytes(&data)?);
    let data_arg = ptb.input(data_input)?;

    let signature_input = CallArg::Pure(bcs::to_bytes(&signature)?);
    let signature_arg = ptb.input(signature_input)?;

    let package_id = ObjectID::from_hex_literal(PACKAGE_ID)?;
    let module = Identifier::new("main")?;
    let function = Identifier::new("user_request_1")?;

    ptb.command(Command::move_call(
        package_id,
        module,
        function,
        vec![], // no type arguments
        vec![user_state_arg, name_arg, data_arg, signature_arg],
    ));

    let builder = ptb.finish();
    let gas_price = client.read_api().get_reference_gas_price().await?;
    let gas_coin = get_gas_coin(client, &sender, GAS_BUDGET).await?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        builder,
        GAS_BUDGET,
        gas_price,
    );

    // Sign transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

    // Execute transaction
    let result = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", result.digest);

    // Print transaction errors if any
    if let Some(effects) = &result.effects {
        if effects.status().is_err() {
            println!("❌ Transaction failed!");
            println!("   Status: {:?}", effects.status());
        } else {
            println!("✅ Transaction succeeded");
        }
    }

    // Extract UserStateEvent from the transaction
    let mut user_state_event = None;
    if let Some(events) = &result.events {
        for event in &events.data {
            if event.type_.name.as_str() == "UserStateEvent" {
                user_state_event = Some(event.parsed_json.clone());
                break;
            }
        }
    }

    Ok((result.digest.to_string(), user_state_event))
}

/// Make a request to the user using BCS serialized UserRequest
/// This tests the new user_request_2 function that accepts BCS bytes and deserializes them
/// Returns the tx digest and the UserStateEvent data
pub async fn user_request_2_with_bcs(
    object_id: String,
    name: String,
    data: U256,
    signature: Vec<u8>,
) -> Result<(String, Option<serde_json::Value>), anyhow::Error> {
    let client = get_sui_client().await?;
    let keypair = get_keypair_from_env()?;
    let sender = SuiAddress::from(&keypair.public());

    let mut keystore = InMemKeystore::default();
    keystore.add_key(Some("sender".to_string()), keypair)?;

    let mut ptb = ProgrammableTransactionBuilder::new();

    // Get the shared UserState object
    let user_state_id = ObjectID::from_hex_literal(&object_id)?;
    let object_info = client
        .read_api()
        .get_object_with_options(
            user_state_id,
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

    let owner = object_info
        .owner()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Object owner not found"))?;
    let object_ref = object_info
        .data
        .ok_or_else(|| anyhow::anyhow!("Object data not found"))?
        .object_ref();

    let obj_arg = match owner {
        Owner::Shared {
            initial_shared_version,
        } => ObjectArg::SharedObject {
            id: object_ref.0,
            initial_shared_version,
            mutable: true,
        },
        _ => return Err(anyhow::anyhow!("Expected shared object")),
    };

    let user_state_arg = ptb.input(CallArg::Object(obj_arg))?;

    // Create UserRequest and serialize it to BCS bytes
    let user_request = UserRequest {
        name: name.clone(),
        data,
        signature: signature.clone(),
    };
    let serialized_request = bcs::to_bytes(&user_request)?;

    println!("📋 Calling user_request_2 with BCS serialized data:");
    println!("   Name: {}", name);
    println!("   Data: {}", data);
    println!("   Signature: {:?}", signature);
    println!(
        "   Serialized BCS bytes ({} bytes): {:02x?}",
        serialized_request.len(),
        serialized_request
    );
    println!("   Hex: {}", hex::encode(&serialized_request));

    // Prepare BCS serialized request argument
    let request_input = CallArg::Pure(bcs::to_bytes(&serialized_request)?);
    let request_arg = ptb.input(request_input)?;

    let package_id = ObjectID::from_hex_literal(PACKAGE_ID)?;
    let module = Identifier::new("main")?;
    let function = Identifier::new("user_request_2")?;

    ptb.command(Command::move_call(
        package_id,
        module,
        function,
        vec![], // no type arguments
        vec![user_state_arg, request_arg],
    ));

    let builder = ptb.finish();
    let gas_price = client.read_api().get_reference_gas_price().await?;
    let gas_coin = get_gas_coin(client, &sender, GAS_BUDGET).await?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        builder,
        GAS_BUDGET,
        gas_price,
    );

    // Sign transaction
    let signature_for_tx = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

    // Execute transaction
    let result = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature_for_tx]),
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", result.digest);

    // Print transaction errors if any
    if let Some(effects) = &result.effects {
        if effects.status().is_err() {
            println!("❌ Transaction failed!");
            println!("   Status: {:?}", effects.status());
        } else {
            println!("✅ Transaction succeeded");
        }
    }

    // Extract UserStateEvent from the transaction
    let mut user_state_event = None;
    if let Some(events) = &result.events {
        for event in &events.data {
            if event.type_.name.as_str() == "UserStateEvent" {
                user_state_event = Some(event.parsed_json.clone());
                break;
            }
        }
    }

    Ok((result.digest.to_string(), user_state_event))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_userrequest_bcs_serialization() {
        // Create the same UserRequest as in the Move test
        let user_request = UserRequest {
            name: "Test User BCS".to_string(),
            data: U256::from(987654321u64),
            signature: vec![11, 22, 33, 44, 55, 66],
        };

        // Serialize using bcs
        let serialized_bytes =
            bcs::to_bytes(&user_request).expect("Failed to serialize UserRequest");

        // The correct BCS format according to Rust BCS library
        println!("=== BCS Serialization Analysis ===");
        println!(
            "Rust BCS serialized bytes ({} bytes):",
            serialized_bytes.len()
        );
        println!("Raw bytes: {:02x?}", serialized_bytes);
        println!("Hex string: {}", hex::encode(&serialized_bytes));

        // Verify the structure manually
        assert_eq!(
            serialized_bytes.len(),
            53,
            "Total should be 53 bytes (1+13+32+1+6)"
        );

        // String part: length + content
        assert_eq!(
            serialized_bytes[0], 0x0d,
            "String length should be 13 (0x0d)"
        );
        assert_eq!(
            &serialized_bytes[1..14],
            b"Test User BCS",
            "String content should match"
        );

        // U256 part: 32 bytes little-endian
        let u256_part = &serialized_bytes[14..46];
        println!("U256 part ({} bytes): {:02x?}", u256_part.len(), u256_part);
        assert_eq!(u256_part.len(), 32, "U256 should be exactly 32 bytes");

        // Verify U256 value: 987654321 = 0x3ade68b1 in little-endian should be [b1, 68, de, 3a, ...]
        assert_eq!(
            u256_part[0..4],
            [0xb1, 0x68, 0xde, 0x3a],
            "U256 should start with little-endian 987654321"
        );
        assert_eq!(
            u256_part[4..32],
            [0u8; 28],
            "Remaining U256 bytes should be zero"
        );

        // Vector part: length + content
        assert_eq!(serialized_bytes[46], 0x06, "Vector length should be 6");
        assert_eq!(
            &serialized_bytes[47..53],
            &[11, 22, 33, 44, 55, 66],
            "Vector content should match"
        );

        println!("✅ BCS serialization structure verified!");

        // Note: This demonstrates the correct BCS format for UserRequest
        // Move's debug output may use a different serialization format or have extra bytes
        println!(
            "📋 Correct BCS format for Sui Move interop: {}",
            hex::encode(&serialized_bytes)
        );

        // This is the format that should be used when passing BCS data to Move functions
        assert!(
            serialized_bytes.len() == 53,
            "This is the correct BCS format for Sui"
        );
    }

    #[test]
    fn test_bcs_format_breakdown() {
        let user_request = UserRequest {
            name: "Test User BCS".to_string(),
            data: U256::from(987654321u64),
            signature: vec![11, 22, 33, 44, 55, 66],
        };

        let serialized = bcs::to_bytes(&user_request).unwrap();

        println!("=== BCS Format Breakdown ===");
        println!("Total length: {} bytes", serialized.len());
        println!("Raw bytes: {:02x?}", serialized);

        // Manual breakdown expected:
        // - String "Test User BCS" (13 chars): 1 byte length (0x0d) + 13 UTF-8 bytes
        // - U256 987654321: 32 bytes little-endian
        // - Vec<u8> [11,22,33,44,55,66]: 1 byte length (0x06) + 6 bytes data

        assert_eq!(
            serialized[0], 0x0d,
            "First byte should be string length (13)"
        );
        assert_eq!(
            &serialized[1..14],
            b"Test User BCS",
            "String content should match"
        );

        // Check U256 (little-endian)
        let u256_bytes = &serialized[14..46];
        let expected_u256_le = 987654321u64.to_le_bytes();
        assert_eq!(
            &u256_bytes[0..8],
            &expected_u256_le,
            "First 8 bytes of U256 should match u64 LE"
        );
        // Remaining 24 bytes should be zero
        assert_eq!(
            &u256_bytes[8..32],
            &[0u8; 24],
            "Remaining U256 bytes should be zero"
        );

        assert_eq!(serialized[46], 0x06, "Vector length should be 6");
        assert_eq!(
            &serialized[47..53],
            &[11, 22, 33, 44, 55, 66],
            "Vector content should match"
        );

        println!("✅ BCS format breakdown verified!");
    }
}

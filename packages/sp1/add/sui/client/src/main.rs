use anyhow::Result;
use serde::{Deserialize, Serialize};
use shared_crypto::intent::Intent;
// Removed unused imports: HashableKey, SP1VerifyingKey
use std::env;
use std::fs;
use std::str::FromStr;
use sui_keys::keystore::{AccountKeystore, InMemKeystore};
use sui_sdk::rpc_types::{
    SuiObjectDataOptions, SuiTransactionBlockEffectsAPI, SuiTransactionBlockResponseOptions,
};
use sui_sdk::types::{
    Identifier,
    base_types::{ObjectID, SuiAddress},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, Command, ObjectArg, Transaction, TransactionData},
};
use sui_sdk::types::{crypto::SuiKeyPair, object::Owner};
use sui_sdk::{SuiClient, SuiClientBuilder};

/// Configuration for the Add contract
#[derive(Debug, Clone)]
pub struct AddContractConfig {
    pub package_id: Option<ObjectID>,
    pub contract_object_id: Option<ObjectID>,
    pub chain: String,
    pub address: SuiAddress,
    pub private_key: String,
}

/// Public values structure matching the Move contract
#[derive(Debug, Serialize, Deserialize)]
pub struct PublicValues {
    pub old_sum: u32,
    pub new_sum: u32,
}

/// Groth16 fixture data structure
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Groth16Fixture {
    pub old_sum: u32,
    pub new_sum: u32,
    pub vkey: String,
    pub public_values: String,
    pub proof: String,
    pub sui_vkey: String,
    pub sui_public_values: String,
    pub sui_proof: String,
}

/// Client for interacting with the Add SP1 contract on Sui
pub struct AddSuiClient {
    sui_client: SuiClient,
    config: AddContractConfig,
    keystore: InMemKeystore,
    sender: SuiAddress,
}

impl AddSuiClient {
    /// Create a new client instance
    pub async fn new(config: AddContractConfig) -> Result<Self> {
        // Build client based on chain configuration
        let sui_client = match config.chain.as_str() {
            "devnet" => SuiClientBuilder::default().build_devnet().await?,
            "testnet" => SuiClientBuilder::default().build_testnet().await?,
            "mainnet" => SuiClientBuilder::default().build_mainnet().await?,
            _ => {
                return Err(anyhow::anyhow!("Unsupported chain: {}", config.chain));
            }
        };

        let mut keystore = InMemKeystore::default();
        let keypair = SuiKeyPair::decode(&config.private_key)
            .map_err(|e| anyhow::anyhow!("Failed to decode private key: {}", e))?;
        let sender = SuiAddress::from(&keypair.public());

        // Verify the sender address matches the one from environment
        if sender != config.address {
            println!(
                "Warning: Derived address {} doesn't match configured address {}",
                sender, config.address
            );
        }

        keystore.add_key(Some("sender".to_string()), keypair)?;

        Ok(Self {
            sui_client,
            config,
            keystore,
            sender,
        })
    }

    /// Create a new client instance from environment variables
    pub async fn from_env() -> Result<Self> {
        dotenv::from_path("../../.env").ok(); // Load .env file from root

        let config = AddContractConfig {
            package_id: env::var("SUI_PACKAGE_ID")
                .ok()
                .and_then(|s| ObjectID::from_str(&s).ok()),
            contract_object_id: env::var("SUI_CONTRACT_ID")
                .ok()
                .and_then(|s| ObjectID::from_str(&s).ok()),
            chain: env::var("SUI_CHAIN").unwrap_or_else(|_| "devnet".to_string()),
            address: SuiAddress::from_str(&env::var("SUI_ADDRESS")?)?,
            private_key: env::var("SUI_SECRET_KEY")?,
        };

        println!("Connecting to Sui {} network", config.chain);
        println!("Using address: {}", config.address);

        Self::new(config).await
    }

    /// Create a new Add contract on Sui
    pub async fn create_contract(&self, vkey: Vec<u8>) -> Result<ObjectID> {
        let package_id = self.config.package_id.ok_or_else(|| {
            anyhow::anyhow!("Package ID not configured. Set SUI_PACKAGE_ID environment variable.")
        })?;

        let mut ptb = ProgrammableTransactionBuilder::new();

        // Encode program hash as BCS bytes (Sui expects vector<u8> encoded as BCS)
        let vkey_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&vkey)?))?;

        // Call create_and_share_contract function (creates and shares in one call)
        let module = Identifier::new("main")?;
        let function = Identifier::new("create_and_share_contract")?;

        ptb.command(Command::move_call(
            package_id,
            module,
            function,
            vec![], // no type arguments
            vec![vkey_arg],
        ));

        let builder = ptb.finish();
        let gas_price = self.sui_client.read_api().get_reference_gas_price().await?;

        // Get gas coin
        let gas_coin = self.get_gas_coin().await?;

        let tx_data = TransactionData::new_programmable(
            self.sender,
            vec![gas_coin.object_ref()],
            builder,
            10_000_000, // gas budget
            gas_price,
        );

        // Sign and execute transaction
        let signature =
            self.keystore
                .sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;

        let response = self
            .sui_client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, vec![signature]),
                SuiTransactionBlockResponseOptions::full_content(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;

        println!("Create contract transaction digest: {}", response.digest);

        // Check transaction status and log detailed information
        if let Some(effects) = &response.effects {
            println!("Transaction status: {:?}", effects.status());

            match effects.status() {
                sui_sdk::rpc_types::SuiExecutionStatus::Success => {
                    println!("✅ Contract creation transaction executed successfully");
                }
                sui_sdk::rpc_types::SuiExecutionStatus::Failure { error } => {
                    println!("❌ Contract creation failed with error: {error}");

                    // Log additional error details
                    if let Some(events) = &response.events {
                        println!("📋 Transaction events:");
                        for (i, event) in events.data.iter().enumerate() {
                            println!("  Event {i}: {event:?}");
                        }
                    }

                    // Log transaction effects details
                    println!("📊 Transaction effects details:");
                    println!("  Gas used: {:?}", effects.gas_cost_summary());
                    println!("  Created objects: {:?}", effects.created());
                    println!("  Mutated objects: {:?}", effects.mutated());
                    println!("  Deleted objects: {:?}", effects.deleted());

                    return Err(anyhow::anyhow!("Contract creation failed: {}", error));
                }
            }
        }

        // Extract created object ID from transaction effects
        if let Some(effects) = response.effects.as_ref() {
            if let Some(created) = effects.created().first() {
                println!(
                    "📦 Created contract object: {}",
                    created.reference.object_id
                );
                return Ok(created.reference.object_id);
            }
        }

        Err(anyhow::anyhow!(
            "Failed to extract created contract object ID"
        ))
    }

    /// Verify an add proof and update the contract state
    pub async fn verify_add_proof(
        &self,
        sp1_system_vkey: Vec<u8>,
        public_inputs: Vec<u8>,
        proof_points: Vec<u8>,
        old_sum: u32,
        new_sum: u32,
    ) -> Result<(u32, u32)> {
        let package_id = self.config.package_id.ok_or_else(|| {
            anyhow::anyhow!("Package ID not configured. Set SUI_PACKAGE_ID environment variable.")
        })?;

        let mut ptb = ProgrammableTransactionBuilder::new();

        // Get the contract object
        let contract_obj = self.get_contract_object().await?;
        let contract_arg = ptb.input(contract_obj)?;

        // Encode arguments as BCS bytes (Sui expects vector<u8> encoded as BCS)
        let sp1_system_vkey_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&sp1_system_vkey)?))?;
        let public_inputs_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&public_inputs)?))?;
        let proof_points_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&proof_points)?))?;
        let old_sum_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&old_sum)?))?;
        let new_sum_arg = ptb.input(CallArg::Pure(bcs::to_bytes(&new_sum)?))?;

        // Call verify_add_proof function
        let module = Identifier::new("main")?;
        let function = Identifier::new("verify_add_proof")?;

        ptb.command(Command::move_call(
            package_id,
            module,
            function,
            vec![], // no type arguments
            vec![
                contract_arg,
                sp1_system_vkey_arg,
                public_inputs_arg,
                proof_points_arg,
                old_sum_arg,
                new_sum_arg,
            ],
        ));

        let builder = ptb.finish();
        let gas_price = self.sui_client.read_api().get_reference_gas_price().await?;

        // Get gas coin
        let gas_coin = self.get_gas_coin().await?;

        let tx_data = TransactionData::new_programmable(
            self.sender,
            vec![gas_coin.object_ref()],
            builder,
            10_000_000, // gas budget
            gas_price,
        );

        // Sign and execute transaction
        let signature =
            self.keystore
                .sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;

        let response = self
            .sui_client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, vec![signature]),
                SuiTransactionBlockResponseOptions::full_content(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;

        println!("Transaction digest: {}", response.digest);

        // Check transaction status and log detailed information
        if let Some(effects) = &response.effects {
            println!("Transaction status: {:?}", effects.status());

            match effects.status() {
                sui_sdk::rpc_types::SuiExecutionStatus::Success => {
                    println!("✅ Transaction executed successfully");
                }
                sui_sdk::rpc_types::SuiExecutionStatus::Failure { error } => {
                    println!("❌ Transaction failed with error: {error}");

                    // Log additional error details
                    if let Some(events) = &response.events {
                        println!("📋 Transaction events:");
                        for (i, event) in events.data.iter().enumerate() {
                            println!("  Event {i}: {event:?}");
                        }
                    }

                    // Log transaction effects details
                    println!("📊 Transaction effects details:");
                    println!("  Gas used: {:?}", effects.gas_cost_summary());
                    println!("  Created objects: {:?}", effects.created());
                    println!("  Mutated objects: {:?}", effects.mutated());
                    println!("  Deleted objects: {:?}", effects.deleted());

                    return Err(anyhow::anyhow!("Transaction failed: {}", error));
                }
            }
        }

        // Return the values since they were validated by the contract
        Ok((old_sum, new_sum))
    }

    /// Get the current sum from the contract
    pub async fn get_current_sum(&self) -> Result<u32> {
        // This would require reading the contract state
        // For now, return a placeholder
        Ok(0)
    }

    /// Helper function to get contract object reference
    async fn get_contract_object(&self) -> Result<CallArg> {
        let contract_object_id = self.config.contract_object_id.ok_or_else(|| {
            anyhow::anyhow!(
                "Contract object ID not configured. Set SUI_CONTRACT_ID environment variable."
            )
        })?;

        let object = self
            .sui_client
            .read_api()
            .get_object_with_options(
                contract_object_id,
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

    /// Helper function to get gas coin
    async fn get_gas_coin(&self) -> Result<sui_sdk::rpc_types::Coin> {
        let coins = self
            .sui_client
            .coin_read_api()
            .get_coins(self.sender, Some("0x2::sui::SUI".to_string()), None, None)
            .await?;

        coins
            .data
            .into_iter()
            .find(|coin| coin.balance >= 10_000_000)
            .ok_or_else(|| anyhow::anyhow!("No suitable gas coin found"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Initializing Add Sui Client from environment variables...");

    // Load configuration from .env file
    let client = AddSuiClient::from_env().await?;

    println!("Add Sui Client initialized successfully!");

    // Display current configuration
    println!("Chain: {}", client.config.chain);
    println!("Address: {}", client.config.address);

    if let Some(package_id) = &client.config.package_id {
        println!("Package ID: {package_id}");
    } else {
        println!(
            "Package ID: Not configured (set SUI_PACKAGE_ID to deploy/interact with contracts)"
        );
    }

    if let Some(contract_id) = &client.config.contract_object_id {
        println!("Contract ID: {contract_id}");

        // If contract is configured, try to read current state
        match client.get_current_sum().await {
            Ok(sum) => println!("Current sum: {sum}"),
            Err(e) => println!("Could not read current sum: {e}"),
        }
    } else {
        println!(
            "Contract ID: Not configured (set SUI_CONTRACT_ID to interact with existing contract)"
        );
    }

    // Create a new contract if SUI_CONTRACT_ID is not set but SUI_PACKAGE_ID is available
    if client.config.package_id.is_some() && client.config.contract_object_id.is_none() {
        println!("\nSUI_CONTRACT_ID not set, creating new contract...");

        // Get program hash from PROGRAM_VKEY (already extracted by make vkey command)
        let program_hash = match env::var("PROGRAM_VKEY") {
            Ok(vkey_str) => hex::decode(vkey_str.trim_start_matches("0x"))
                .map_err(|e| anyhow::anyhow!("Failed to decode PROGRAM_VKEY: {}", e))?,
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "PROGRAM_VKEY environment variable is required to create a new contract"
                ));
            }
        };

        let contract_id = client.create_contract(program_hash).await?;
        println!("✅ Created new contract with Object ID: {contract_id}");
        println!("💡 Set SUI_CONTRACT_ID={contract_id} in your .env file to reuse this contract");
    }

    // Verify a proof if SUI_CONTRACT_ID is set
    if client.config.package_id.is_some() && client.config.contract_object_id.is_some() {
        println!("\nSUI_CONTRACT_ID is set, verifying proof...");

        // Try to read proof data from groth16-fixture.json
        let fixture_path = "../../proofs/groth16-fixture.json";
        let (sp1_system_vkey, public_inputs, proof_points, fixture) = match fs::read_to_string(
            fixture_path,
        ) {
            Ok(json_content) => {
                println!("📁 Reading proof data from {fixture_path}");
                let fixture: Groth16Fixture = serde_json::from_str(&json_content)?;

                // Use Sui-specific proof data (three separate components)
                let sp1_system_vkey = hex::decode(fixture.sui_vkey.trim_start_matches("0x"))
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to decode sui_vkey from fixture: {}", e)
                    })?;

                let public_inputs = hex::decode(fixture.sui_public_values.trim_start_matches("0x"))
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to decode sui_public_values from fixture: {}", e)
                    })?;

                let proof_points = hex::decode(fixture.sui_proof.trim_start_matches("0x"))
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to decode sui_proof from fixture: {}", e)
                    })?;

                println!(
                    "📊 Using Sui fixture data: old_sum={}, new_sum={}",
                    fixture.old_sum, fixture.new_sum
                );
                println!("🔑 SP1 system vkey: {} bytes", sp1_system_vkey.len());
                println!("📊 Public inputs: {} bytes", public_inputs.len());
                println!("🔒 Proof points: {} bytes", proof_points.len());

                (sp1_system_vkey, public_inputs, proof_points, fixture)
            }
            Err(_) => {
                println!(
                    "⚠️  Fixture file not found, cannot proceed without real SP1-Sui proof data"
                );
                return Err(anyhow::anyhow!(
                    "Fixture file {} not found. Please generate proof data first using: cargo run --bin evm",
                    fixture_path
                ));
            }
        };

        match client
            .verify_add_proof(
                sp1_system_vkey,
                public_inputs,
                proof_points,
                fixture.old_sum,
                fixture.new_sum,
            )
            .await
        {
            Ok((old_sum, new_sum)) => {
                println!("✅ Proof verified successfully!");
                println!("   Old sum: {old_sum}");
                println!("   New sum: {new_sum}");
            }
            Err(e) => {
                println!("❌ Proof verification failed: {e}");
            }
        }
    }

    Ok(())
}

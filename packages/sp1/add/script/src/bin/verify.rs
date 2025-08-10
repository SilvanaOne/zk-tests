use alloy::{
    network::EthereumWallet, primitives::Address, providers::ProviderBuilder,
    signers::local::PrivateKeySigner, sol,
};
use clap::Parser;
use dotenv::dotenv;
use std::{env, fs};
use add_script::verify::{verify_core_proof_from_file, verify_core_proof_from_json, CoreProofFixture};
use sp1_sdk::include_elf;
use std::path::PathBuf;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    Add,
    "abi/Add.json"
);

/// The ELF files for the programs
pub const ADD_ELF: &[u8] = include_elf!("add-program");
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

#[derive(Parser, Debug)]
#[command(about = "Verify an Add proof on Sepolia or locally")]
struct Args {
    #[arg(long, help = "Path to the proof fixture JSON file")]
    proof_file: Option<String>,

    #[arg(long, default_value = "groth16", help = "Proof type: groth16, plonk, or core")]
    proof_type: String,
    
    #[arg(long, help = "Use aggregate ELF for core proof verification")]
    aggregate: bool,
    
    #[arg(long, help = "Skip verification key check (useful for debugging)")]
    skip_vkey_check: bool,
    
    #[arg(long, help = "Simulate on-chain verification (no ELF required)")]
    onchain: bool,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();

    let args = Args::parse();

    // Validate proof type
    if args.proof_type != "groth16" && args.proof_type != "plonk" && args.proof_type != "core" {
        eprintln!("Error: proof_type must be 'groth16', 'plonk', or 'core'");
        std::process::exit(1);
    }
    
    // Handle core proof verification separately
    if args.proof_type == "core" {
        // Setup logger for core proof
        sp1_sdk::utils::setup_logger();
        
        // Get the proof file path
        let proof_path = if let Some(path) = args.proof_file {
            PathBuf::from(path)
        } else {
            // Find the most recent core proof
            println!("No proof file specified, using the most recent core proof...");
            add_script::verify::find_latest_core_proof()
                .map_err(|e| eyre::eyre!("Failed to find latest core proof: {}", e))?
        };
        
        // Check if we're doing on-chain simulation
        if args.onchain {
            // Load the JSON and verify without ELF
            println!("Simulating on-chain verification (no ELF required)...");
            let json_str = fs::read_to_string(&proof_path)
                .map_err(|e| eyre::eyre!("Failed to read proof file: {}", e))?;
            let fixture: CoreProofFixture = serde_json::from_str(&json_str)
                .map_err(|e| eyre::eyre!("Failed to parse proof JSON: {}", e))?;
            
            verify_core_proof_from_json(&fixture)
                .map_err(|e| eyre::eyre!("Failed to verify proof: {}", e))?;
        } else {
            // Regular verification with ELF
            // Determine which ELF to use
            let elf = if args.aggregate {
                println!("Using aggregate program ELF for verification");
                AGGREGATE_ELF
            } else {
                println!("Using add program ELF for verification");
                ADD_ELF
            };
            
            // Verify the core proof
            verify_core_proof_from_file(&proof_path, elf, args.skip_vkey_check)
                .map_err(|e| eyre::eyre!("Failed to verify core proof: {}", e))?;
        }
        return Ok(());
    }

    // Set default fixture file based on proof type
    let default_fixture = match args.proof_type.as_str() {
        "groth16" => "../proofs/groth16-fixture.json",
        "plonk" => "../proofs/plonk-fixture.json",
        _ => unreachable!(),
    };

    let proof_file = args
        .proof_file
        .unwrap_or_else(|| default_fixture.to_string());

    // Load environment variables
    let rpc_url = env::var("RPC_URL").expect("RPC_URL not set in .env");
    let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set in .env");

    // Get contract address based on proof type
    let contract_address_var = match args.proof_type.as_str() {
        "groth16" => "CONTRACT_ADDRESS_GROTH16",
        "plonk" => "CONTRACT_ADDRESS_PLONK",
        _ => unreachable!(),
    };
    let contract_address = env::var(contract_address_var)
        .unwrap_or_else(|_| panic!("{contract_address_var} not set in .env"));

    // Parse contract address
    let contract_address: Address = contract_address.parse()?;

    // Set up wallet and provider
    let signer: PrivateKeySigner = private_key.parse()?;
    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse()?);

    // Load proof data from JSON file
    let proof_data = fs::read_to_string(&proof_file)?;
    let proof_json: serde_json::Value = serde_json::from_str(&proof_data)?;

    // Extract proof and public values
    let proof_hex = proof_json["proof"].as_str().expect("proof field missing");
    let public_values_hex = proof_json["publicValues"]
        .as_str()
        .expect("publicValues field missing");

    // Remove 0x prefix and convert to bytes
    let proof_bytes = hex::decode(&proof_hex[2..])?;
    let public_values_bytes = hex::decode(&public_values_hex[2..])?;

    println!("Proof type: {}", args.proof_type.to_uppercase());
    println!("Contract address: {contract_address}");
    println!("Proof file: {proof_file}");
    println!("Proof length: {} bytes", proof_bytes.len());
    println!("Public values length: {} bytes", public_values_bytes.len());

    // Create contract instance
    let contract = Add::new(contract_address, &provider);

    // Call verifyAddProof
    println!("Calling verifyAddProof...");

    let call_builder = contract.verifyAddProof(public_values_bytes.into(), proof_bytes.into());

    match call_builder.call().await {
        Ok(result) => {
            println!("✅ Proof verification successful!");
            println!("Results: old_sum={}, new_sum={}", result._0, result._1);

            // Now send the transaction to get the tx hash
            println!("Sending transaction...");
            let pending_tx = call_builder.send().await?;
            let tx_hash = *pending_tx.tx_hash();
            println!("Transaction hash: {tx_hash:?}");

            // Wait for transaction receipt
            let receipt = pending_tx.get_receipt().await?;
            println!("Transaction confirmed in block: {:?}", receipt.block_number);
            println!("Gas used: {:?}", receipt.gas_used);
        }
        Err(e) => {
            println!("❌ Proof verification failed: {e}");
            return Err(e.into());
        }
    }

    Ok(())
}

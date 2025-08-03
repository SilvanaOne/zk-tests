use alloy::{
    network::EthereumWallet, primitives::Address, providers::ProviderBuilder,
    signers::local::PrivateKeySigner, sol,
};
use clap::Parser;
use dotenv::dotenv;
use std::{env, fs};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    Add,
    "abi/Add.json"
);

#[derive(Parser, Debug)]
#[command(about = "Verify an Add proof on Sepolia")]
struct Args {
    #[arg(long, help = "Path to the proof fixture JSON file")]
    proof_file: Option<String>,

    #[arg(long, default_value = "groth16", help = "Proof type: groth16 or plonk")]
    proof_type: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();

    let args = Args::parse();

    // Validate proof type
    if args.proof_type != "groth16" && args.proof_type != "plonk" {
        eprintln!("Error: proof_type must be either 'groth16' or 'plonk'");
        std::process::exit(1);
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

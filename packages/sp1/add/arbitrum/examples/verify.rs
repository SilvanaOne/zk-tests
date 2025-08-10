//! Example on how to interact with the deployed SP1 Add Verifier contract.
//! This example loads a Groth16 proof from the fixtures and verifies it on-chain.
//! 
//! NOTE: Currently using a MOCK verification since Stylus doesn't support BN254 precompiles.
//! The contract will accept any proof and just parse the public values to update state.

use dotenv::dotenv;
use ethers::{
    middleware::SignerMiddleware,
    prelude::abigen,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::Address,
};
use eyre::eyre;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

/// Your private key
const PRIVATE_KEY: &str = "ARBITRUM_PRIVATE_KEY";

/// Stylus RPC endpoint url.
const RPC_URL: &str = "ARBITRUM_RPC_URL";

/// Deployed program address.
const STYLUS_CONTRACT_ADDRESS: &str = "STYLUS_CONTRACT_ADDRESS";

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv().ok();
    
    let private_key =
        std::env::var(PRIVATE_KEY).map_err(|_| eyre!("No {} env var set", PRIVATE_KEY))?;
    let rpc_url = std::env::var(RPC_URL).map_err(|_| eyre!("No {} env var set", RPC_URL))?;
    let contract_address = std::env::var(STYLUS_CONTRACT_ADDRESS)
        .map_err(|_| eyre!("No {} env var set", STYLUS_CONTRACT_ADDRESS))?;
    
    // Load the proof fixture
    let fixture_path = Path::new("../proofs/groth16-fixture.json");
    let fixture_content = fs::read_to_string(fixture_path)
        .map_err(|e| eyre!("Failed to read fixture file: {}", e))?;
    
    // Simple JSON parsing for the fields we need
    let get_json_field = |content: &str, field: &str| -> Option<String> {
        let pattern = format!("\"{}\":", field);
        content.find(&pattern).and_then(|start| {
            let value_start = content[start + pattern.len()..].find('\"')? + start + pattern.len() + 1;
            let value_end = content[value_start..].find('\"')? + value_start;
            Some(content[value_start..value_end].to_string())
        })
    };
    
    let vkey = get_json_field(&fixture_content, "vkey")
        .ok_or_else(|| eyre!("Failed to parse vkey from fixture"))?;
    let old_root = get_json_field(&fixture_content, "oldRoot")
        .ok_or_else(|| eyre!("Failed to parse oldRoot from fixture"))?;
    let new_root = get_json_field(&fixture_content, "newRoot")
        .ok_or_else(|| eyre!("Failed to parse newRoot from fixture"))?;
    let arbitrum_proof = get_json_field(&fixture_content, "arbitrumProof")
        .ok_or_else(|| eyre!("Failed to parse arbitrumProof from fixture"))?;
    let arbitrum_public_values = get_json_field(&fixture_content, "arbitrumPublicValues")
        .ok_or_else(|| eyre!("Failed to parse arbitrumPublicValues from fixture"))?;
    
    println!("Loaded proof fixture:");
    println!("  Old Root: {}", old_root);
    println!("  New Root: {}", new_root);
    println!("  VKey: {}", vkey);
    
    // Define the contract ABI (using uint8[] as per Stylus export)
    abigen!(
        AddVerifier,
        r#"[
            function verifyProof(bytes32 vkey, uint8[] calldata publicValues, uint8[] calldata proofBytes) external returns (uint256, uint256)
        ]"#
    );

    let provider = Provider::<Http>::try_from(rpc_url)?;
    let address: Address = contract_address.parse()?;

    let wallet = LocalWallet::from_str(&private_key)?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.clone().with_chain_id(chain_id),
    ));

    let contract = AddVerifier::new(address, client);
    
    // Parse the vkey from the fixture (remove 0x prefix)
    let vkey_hex = vkey.trim_start_matches("0x");
    let vkey_bytes: [u8; 32] = hex::decode(vkey_hex)?
        .try_into()
        .map_err(|_| eyre!("Invalid vkey length"))?;
    
    // Prepare the proof and public values (using Arbitrum-specific fields)
    let proof_hex = arbitrum_proof.trim_start_matches("0x");
    let proof_bytes = hex::decode(proof_hex)?;
    
    let public_values_hex = arbitrum_public_values.trim_start_matches("0x");
    let public_values_bytes = hex::decode(public_values_hex)?;
    
    println!("\nSubmitting proof for verification...");
    println!("  Proof size: {} bytes", proof_bytes.len());
    println!("  Public values size: {} bytes", public_values_bytes.len());
    
    // Call verifyProof (convert bytes to Vec<u8> for uint8[] parameter)
    let tx = contract.verify_proof(
        vkey_bytes,
        public_values_bytes.clone(),
        proof_bytes.clone(),
    );
    
    match tx.send().await {
        Ok(pending_tx) => {
            println!("Transaction submitted: {:?}", pending_tx.tx_hash());
            
            if let Some(receipt) = pending_tx.await? {
                println!("Transaction receipt: {:?}", receipt);
                
                if receipt.status == Some(1.into()) {
                    println!("✅ Proof verification succeeded!");
                    
                    // Check for events
                    if !receipt.logs.is_empty() {
                        println!("Events emitted:");
                        for log in &receipt.logs {
                            println!("  Event: {:?}", log);
                        }
                    }
                } else {
                    println!("❌ Transaction failed");
                }
            }
        }
        Err(e) => {
            println!("❌ Error submitting transaction: {}", e);
            
            // Try to decode the error
            if let Some(revert_data) = e.as_revert() {
                println!("Revert data: 0x{}", hex::encode(revert_data));
            }
        }
    }
    
    Ok(())
}
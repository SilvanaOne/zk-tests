use std::str::FromStr;
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig, pubkey::Pubkey, signature::read_keypair_file,
        compute_budget::ComputeBudgetInstruction,
    },
    Client, Cluster,
};
use add::{SP1Groth16Proof};

#[test]
fn test_verify_sp1_proof() {
    // Load the groth16 fixture
    let fixture_path = "../../proofs/groth16-fixture.json";
    let fixture_content = std::fs::read_to_string(fixture_path)
        .expect("Failed to read groth16 fixture file");
    let fixture: serde_json::Value = serde_json::from_str(&fixture_content)
        .expect("Failed to parse groth16 fixture");

    // Extract Solana-specific proof data
    let solana_proof_hex = fixture["solanaProof"].as_str()
        .expect("Missing solanaProof in fixture");
    let solana_public_inputs_hex = fixture["solanaPublicInputs"].as_str()
        .expect("Missing solanaPublicInputs in fixture");

    // Remove "0x" prefix and decode hex
    let proof_bytes = hex::decode(&solana_proof_hex[2..])
        .expect("Failed to decode proof hex");
    let public_inputs_bytes = hex::decode(&solana_public_inputs_hex[2..])
        .expect("Failed to decode public inputs hex");

    // Create SP1Groth16Proof struct
    let proof_data = SP1Groth16Proof {
        proof: proof_bytes,
        sp1_public_inputs: public_inputs_bytes,
    };

    // Setup Anchor client
    let program_id = "BTbMTALLVaSor7BfPTgDoFJvqmMAePHgs6HdZRdv4B1x";
    let anchor_wallet = std::env::var("ANCHOR_WALLET")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            format!("{}/.config/solana/id.json", home)
        });
    let payer = read_keypair_file(&anchor_wallet).unwrap();

    let client = Client::new_with_options(Cluster::Localnet, &payer, CommitmentConfig::confirmed());
    let program_id = Pubkey::from_str(program_id).unwrap();
    let program = client.program(program_id).unwrap();

    // Call verify_add_proof instruction with increased compute budget
    // SP1 proof verification requires more compute units than the default
    let tx = program
        .request()
        .instruction(ComputeBudgetInstruction::set_compute_unit_limit(300_000))
        .accounts(add::accounts::VerifyProof {})
        .args(add::instruction::VerifyAddProof { proof_data })
        .send()
        .expect("Failed to send transaction");

    println!("SP1 proof verification transaction signature: {}", tx);
    
    // Get the RPC client from the anchor client to fetch logs
    let rpc_client = program.rpc();
    
    // Small delay to ensure transaction is confirmed
    std::thread::sleep(std::time::Duration::from_millis(1000));
    
    // Fetch the actual transaction details from RPC
    use anchor_client::solana_client::rpc_config::RpcTransactionConfig;
    
    let config = RpcTransactionConfig {
        encoding: None,
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: None,
    };
    
    match rpc_client.get_transaction_with_config(&tx, config) {
        Ok(confirmed_tx) => {
            if let Some(meta) = &confirmed_tx.transaction.meta {
                println!("\n=== Transaction Details ===");
                
                // Transaction status
                if meta.err.is_none() {
                    println!("Status: Success ✅");
                } else {
                    println!("Status: Failed ❌");
                    if let Some(err) = &meta.err {
                        println!("Error: {:?}", err);
                    }
                }
                
                // Compute units consumed
                match &meta.compute_units_consumed {
                    solana_transaction_status::option_serializer::OptionSerializer::Some(units) => {
                        println!("Compute Units Consumed: {}", units);
                    }
                    _ => {
                        println!("Compute Units Consumed: Not available");
                    }
                }
                
                // Transaction fee
                println!("Fee: {} lamports", meta.fee);
                
                // Transaction logs
                println!("\n--- Transaction Logs ---");
                match &meta.log_messages {
                    solana_transaction_status::option_serializer::OptionSerializer::Some(logs) => {
                        for log in logs {
                            println!("{}", log);
                        }
                    }
                    _ => {
                        println!("No logs available");
                    }
                }
                
                println!("========================\n");
            } else {
                println!("No transaction metadata available");
            }
        }
        Err(e) => {
            println!("Warning: Could not fetch transaction details: {}", e);
        }
    }
    
    println!("SP1 proof verified successfully on Solana!");
}
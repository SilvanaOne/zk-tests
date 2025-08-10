//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can have an
//! EVM-Compatible proof generated which can be verified on-chain.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release --bin evm -- --system groth16
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release --bin evm -- --system plonk
//! ```

use add_lib::PublicValuesStruct;
use add_script::map::{AccountManager, AccountOperation};
use add_script::proof::{self, FinalProofType};
use add_script::solana::create_solana_fixture;
use add_script::sui::convert_sp1_proof_for_sui;
use alloy_sol_types::SolType;
use clap::{Parser, ValueEnum};
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use sp1_sdk::{
    HashableKey, ProverClient, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey, include_elf,
};
use std::path::PathBuf;
use std::time::Instant;

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

/// The arguments for the EVM command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct EVMArgs {
    #[arg(long, alias = "length", default_value = "10")]
    operations: u32,
    #[arg(long, value_enum, default_value = "groth16")]
    system: ProofSystem,
    #[arg(long, default_value = "2")]
    proofs: u32,
}

/// Enum representing the available proof systems
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum ProofSystem {
    Plonk,
    Groth16,
}

/// A fixture that can be used to test the verification of SP1 zkVM proofs inside Solidity, Sui, Solana, and Arbitrum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SP1AddProofFixture {
    // Common fields
    old_root: String,
    new_root: String,

    // Ethereum fields (keep existing for backward compatibility)
    vkey: String,
    public_values: String,
    proof: String,

    // Sui-specific fields
    sui_vkey: String,
    sui_public_values: String,
    sui_proof: String,

    // Solana-specific fields
    solana_vkey_hash: String,
    solana_proof: String,
    solana_public_inputs: String,
    
    // Arbitrum-specific fields (for sp1-verifier crate)
    arbitrum_proof: String,
    arbitrum_public_values: String,
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = EVMArgs::parse();

    // Setup the prover client.
    //let client = ProverClient::from_env();

    if args.proofs == 1 {
        // Single proof mode
        generate_single_evm_proof(&args);
    } else {
        // Multiple proofs with aggregation
        generate_aggregated_evm_proofs(&args);
    }
}

fn generate_single_evm_proof(args: &EVMArgs) {
    let client = ProverClient::from_env();
    println!("Setting up proving keys...");
    let setup_start = Instant::now();
    let (_pk, vk) = client.setup(ADD_ELF);
    let setup_duration = setup_start.elapsed();
    println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

    // Generate random account operations
    let mut rng = thread_rng();
    let operations: Vec<AccountOperation> = (0..args.operations)
        .map(|_| {
            let account_num = rng.gen_range(1..=100);
            let add_value = rng.gen_range(1..=1000);
            AccountOperation::new(account_num, add_value)
        })
        .collect();

    println!("Generated {} account operations", operations.len());

    // Create account manager and process operations
    let mut manager = AccountManager::new(16);
    let initial_root = manager.get_root();

    // Setup the inputs for zkVM
    let mut stdin = SP1Stdin::new();

    // Write initial root as [u8; 32]
    let root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
    stdin.write(&root_bytes);

    // Write number of operations
    stdin.write(&args.operations);

    // Process operations and collect actions
    let mut actions = Vec::new();
    for op in &operations {
        let action = manager
            .process_action(op.account_num, op.add_value)
            .expect("Failed to process action");

        // Serialize the action for zkVM
        let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
        stdin.write(&action_bytes);

        actions.push(action);
    }

    let final_root = manager.get_root();
    println!("\nInitial root: 0x{}", hex::encode(initial_root.as_bytes()));
    println!("Final root: 0x{}", hex::encode(final_root.as_bytes()));
    println!("  Proof System: {:?}", args.system);

    // Generate the proof based on the selected proof system.
    let final_proof_type = match args.system {
        ProofSystem::Plonk => FinalProofType::Plonk,
        ProofSystem::Groth16 => FinalProofType::Groth16,
    };

    let proof = proof::generate_single_proof(ADD_ELF, &stdin, final_proof_type)
        .expect("failed to generate proof");

    proof::print_proof_statistics(&proof);

    let verify_duration = proof::verify_proof(&proof, &vk).expect("failed to verify proof");

    // Print performance summary
    println!("\n=== Performance Summary ===");
    println!("Setup time:      {:.2}s", setup_duration.as_secs_f64());
    println!("Verification:    {:.2}s", verify_duration.as_secs_f64());

    create_proof_fixture(&proof, &vk, args.system);
}

fn generate_aggregated_evm_proofs(args: &EVMArgs) {
    let final_proof_type = match args.system {
        ProofSystem::Plonk => FinalProofType::Plonk,
        ProofSystem::Groth16 => FinalProofType::Groth16,
    };

    let result = proof::generate_and_aggregate_proofs(
        ADD_ELF,
        AGGREGATE_ELF,
        args.proofs,
        args.operations,
        final_proof_type,
    )
    .expect("failed to generate and aggregate proofs");

    println!("Successfully generated aggregated {:?} proof!", args.system);

    proof::print_proof_statistics(&result.proof);
    proof::print_aggregated_results(&result.aggregated_values);
    proof::print_aggregation_performance_summary(&result);

    create_proof_fixture(&result.proof, &result.aggregate_vk, args.system);
}

/// Create a fixture for the given proof.
fn create_proof_fixture(
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
    system: ProofSystem,
) {
    // Deserialize the public values.
    let bytes = proof.public_values.as_slice();
    let public_values = PublicValuesStruct::abi_decode(bytes).unwrap();
    let old_root_hex = format!(
        "0x{}",
        hex::encode(public_values.old_root.to_be_bytes::<32>())
    );
    let new_root_hex = format!(
        "0x{}",
        hex::encode(public_values.new_root.to_be_bytes::<32>())
    );

    // Generate Sui-compatible proof data (only for Groth16)
    let (sui_vkey, sui_public_values, sui_proof) = if system == ProofSystem::Groth16 {
        match convert_sp1_proof_for_sui(proof.clone()) {
            Ok(sui_proof_data) => (
                format!("0x{}", hex::encode(sui_proof_data.vkey_bytes)),
                format!("0x{}", hex::encode(sui_proof_data.public_inputs_bytes)),
                format!("0x{}", hex::encode(sui_proof_data.proof_bytes)),
            ),
            Err(e) => {
                eprintln!("Warning: Failed to convert proof for Sui: {e}");
                ("".to_string(), "".to_string(), "".to_string())
            }
        }
    } else {
        ("".to_string(), "".to_string(), "".to_string())
    };

    // Generate Solana-compatible proof data (only for Groth16)
    let (solana_vkey_hash, solana_proof, solana_public_inputs) = if system == ProofSystem::Groth16 {
        let vkey_hash = vk.bytes32();
        match create_solana_fixture(proof, &vkey_hash) {
            Ok(solana_fixture) => (
                solana_fixture.vkey_hash,
                format!("0x{}", hex::encode(solana_fixture.proof_bytes)),
                format!("0x{}", hex::encode(solana_fixture.public_inputs_bytes)),
            ),
            Err(e) => {
                eprintln!("Warning: Failed to convert proof for Solana: {e}");
                ("".to_string(), "".to_string(), "".to_string())
            }
        }
    } else {
        ("".to_string(), "".to_string(), "".to_string())
    };

    // Generate Arbitrum-specific proof data (for sp1-verifier crate)
    // The sp1-verifier expects the raw proof bytes with the 4-byte prefix
    // The proof.bytes() already includes this prefix
    let arbitrum_proof = format!("0x{}", hex::encode(proof.bytes()));
    let arbitrum_public_values = format!("0x{}", hex::encode(bytes));

    // Create the testing fixture so we can test things end-to-end.
    let fixture = SP1AddProofFixture {
        // Common fields
        old_root: old_root_hex,
        new_root: new_root_hex,

        // Ethereum fields (keep existing for backward compatibility)
        vkey: vk.bytes32().to_string(),
        public_values: format!("0x{}", hex::encode(bytes)),
        proof: format!("0x{}", hex::encode(proof.bytes())),

        // Sui-specific fields
        sui_vkey,
        sui_public_values,
        sui_proof,

        // Solana-specific fields
        solana_vkey_hash,
        solana_proof,
        solana_public_inputs,
        
        // Arbitrum-specific fields
        arbitrum_proof,
        arbitrum_public_values,
    };

    // The verification key is used to verify that the proof corresponds to the execution of the
    // program on the given input.
    //
    // Note that the verification key stays the same regardless of the input.
    println!("Verification Key: {}", fixture.vkey);

    // The public values are the values which are publicly committed to by the zkVM.
    //
    // If you need to expose the inputs or outputs of your program, you should commit them in
    // the public values.
    println!("Public Values: {}", fixture.public_values);

    // The proof proves to the verifier that the program was executed with some inputs that led to
    // the give public values.
    println!("Proof Bytes: {}", fixture.proof);

    // Print Sui-specific data if available
    if !fixture.sui_vkey.is_empty() {
        println!("\n--- Sui-specific data ---");
        println!("Sui Verification Key: {}", fixture.sui_vkey);
        println!("Sui Public Values: {}", fixture.sui_public_values);
        println!("Sui Proof Bytes: {}", fixture.sui_proof);
    }

    // Print Solana-specific data if available
    if !fixture.solana_vkey_hash.is_empty() {
        println!("\n--- Solana-specific data ---");
        println!("Solana Verification Key Hash: {}", fixture.solana_vkey_hash);
        println!("Solana Public Inputs: {}", fixture.solana_public_inputs);
        println!("Solana Proof Bytes: {}", fixture.solana_proof);
    }
    
    // Print Arbitrum-specific data
    if !fixture.arbitrum_proof.is_empty() {
        println!("\n--- Arbitrum-specific data (sp1-verifier) ---");
        println!("Arbitrum Proof Bytes: {}", fixture.arbitrum_proof);
        println!("Arbitrum Public Values: {}", fixture.arbitrum_public_values);
    }

    // Save the fixture to a file.
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");
    std::fs::create_dir_all(&fixture_path).expect("failed to create fixture path");
    std::fs::write(
        fixture_path.join(format!("{system:?}-fixture.json").to_lowercase()),
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .expect("failed to write fixture");
}

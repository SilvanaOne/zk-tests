//! Shared proof generation and aggregation utilities

use crate::map::{AccountManager, AccountOperation};
use add_lib::PublicValuesStruct;
use alloy_sol_types::{SolType, private::U256};
use hex;
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use sp1_sdk::{
    HashableKey, Prover, ProverClient, SP1Proof, SP1ProofWithPublicValues, SP1Stdin,
    SP1VerifyingKey,
    install::{groth16_circuit_artifacts_dir, plonk_circuit_artifacts_dir, install_circuit_artifacts},
};
use sp1_core_executor::{SP1Context, SP1ReduceProof};
use sp1_prover::{components::CpuProverComponents, InnerSC, SP1Prover};
use sp1_stark::SP1ProverOpts;
use sp1_primitives::io::SP1PublicValues;
use std::path::PathBuf;
use std::time::Instant;

/// Enum representing the available proof types for final proof generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalProofType {
    Core,
    Groth16,
    Plonk,
}

/// Ensure circuit artifacts are downloaded before proof generation
///
/// SP1's try_install_circuit_artifacts() only checks if the directory exists,
/// not if the actual circuit files are present. This function checks for the
/// required circuit binary file and triggers download if missing.
fn ensure_circuit_artifacts(proof_type: FinalProofType) -> Result<(), Box<dyn std::error::Error>> {
    let (artifacts_type, build_dir) = match proof_type {
        FinalProofType::Groth16 => {
            ("groth16", groth16_circuit_artifacts_dir())
        }
        FinalProofType::Plonk => {
            ("plonk", plonk_circuit_artifacts_dir())
        }
        FinalProofType::Core => return Ok(()), // Compressed proofs don't need circuits
    };

    // Check if the required circuit file exists
    let circuit_file = build_dir.join(format!("{}_circuit.bin", artifacts_type));

    if circuit_file.exists() {
        println!("[sp1] {} circuit artifacts verified at {}", artifacts_type, build_dir.display());
        Ok(())
    } else {
        // Clean up empty directory if it exists
        if build_dir.exists() {
            println!("[sp1] {} circuit directory exists but files are missing. Cleaning up...", artifacts_type);
            std::fs::remove_dir_all(&build_dir)?;
        }

        // Download the circuit artifacts
        println!("[sp1] Downloading {} circuit artifacts (~4GB, may take a few minutes)...", artifacts_type);
        install_circuit_artifacts(build_dir.clone(), artifacts_type);
        println!("[sp1] {} circuit artifacts downloaded successfully", artifacts_type);

        Ok(())
    }
}

/// Core proof fixture for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreProofFixture {
    pub old_root: String,
    pub new_root: String,
    pub vkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vkey_bytes: Option<String>,  // Serialized SP1VerifyingKey for actual verification
    pub public_values: String,
    pub proof: String,
}

/// Result of proof aggregation containing the proof and timing information
pub struct AggregationResult {
    pub proof: SP1ProofWithPublicValues,
    pub setup_duration: std::time::Duration,
    pub individual_prove_duration: std::time::Duration,
    pub aggregate_duration: std::time::Duration,
    pub verify_duration: std::time::Duration,
    pub aggregated_values: PublicValuesStruct,
    pub aggregate_vk: SP1VerifyingKey,
}

/// Generate multiple proofs and aggregate them with IndexedMerkleMap
pub fn generate_and_aggregate_proofs(
    add_elf: &[u8],
    aggregate_elf: &[u8],
    num_proofs: u32,
    operations_per_proof: u32,
    final_proof_type: FinalProofType,
    use_hosted: bool,
) -> Result<AggregationResult, Box<dyn std::error::Error>> {
    println!("Generating {} proofs for aggregation...", num_proofs);

    // Setup proving keys for both add and aggregate programs
    println!("Setting up proving keys...");
    let setup_start = Instant::now();

    // Set environment variable to control prover type for aggregation
    if use_hosted {
        unsafe {
            std::env::set_var("SP1_PROVER", "network");
        }
    } else {
        unsafe {
            std::env::set_var("SP1_PROVER", "cpu");
        }
    }

    let add_client = ProverClient::builder().cpu().build();
    let aggregate_client = ProverClient::from_env();
    let (add_pk, add_vk) = add_client.setup(add_elf);
    let (aggregate_pk, aggregate_vk) = aggregate_client.setup(aggregate_elf);
    let setup_duration = setup_start.elapsed();
    println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

    // Initialize account manager
    let mut manager = AccountManager::new(16); // Height 16 supports up to 32K accounts
    let _initial_root = manager.get_root();

    // Generate multiple proofs with chained roots
    let mut proofs = Vec::new();
    let prove_start = Instant::now();
    let mut rng = thread_rng();

    for i in 0..num_proofs {
        println!("\nGenerating proof {} of {}...", i + 1, num_proofs);

        // Get current root before operations
        let proof_initial_root = manager.get_root();

        // Generate random account operations for this proof
        let operations: Vec<AccountOperation> = (0..operations_per_proof)
            .map(|_| {
                let account_num = rng.gen_range(1..=100);
                let add_value = rng.gen_range(1..=1000);
                AccountOperation::new(account_num, add_value)
            })
            .collect();

        println!("  Processing {} operations", operations.len());

        // Setup inputs for this proof
        let mut stdin = SP1Stdin::new();

        // Write initial root
        let root_bytes: [u8; 32] = proof_initial_root.as_bytes().try_into().unwrap();
        stdin.write(&root_bytes);

        // Write number of operations
        stdin.write(&operations_per_proof);

        // Process operations and write actions
        for op in &operations {
            let action = manager
                .process_action(op.account_num, op.add_value)
                .expect("Failed to process action");

            // Serialize the action for zkVM
            let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
            stdin.write(&action_bytes);
        }

        let proof_final_root = manager.get_root();
        println!(
            "  Old root: 0x{}",
            hex::encode(proof_initial_root.as_bytes())
        );
        println!("  New root: 0x{}", hex::encode(proof_final_root.as_bytes()));

        // Generate compressed proof for aggregation
        let proof = add_client
            .prove(&add_pk, &stdin)
            .compressed()
            .run()
            .expect("failed to generate proof");

        // Verify the proof output matches expected
        let decoded = PublicValuesStruct::abi_decode(proof.public_values.as_slice())
            .expect("failed to decode public values");

        let old_bytes: [u8; 32] = proof_initial_root.as_bytes().try_into().unwrap();
        let expected_old = U256::from_be_bytes(old_bytes);
        let new_bytes: [u8; 32] = proof_final_root.as_bytes().try_into().unwrap();
        let expected_new = U256::from_be_bytes(new_bytes);

        assert_eq!(
            decoded.old_root, expected_old,
            "Old root mismatch in proof {}",
            i
        );
        assert_eq!(
            decoded.new_root, expected_new,
            "New root mismatch in proof {}",
            i
        );

        proofs.push((proof, add_vk.clone()));
    }

    let individual_prove_duration = prove_start.elapsed();
    println!(
        "\nGenerated {} individual proofs in {:.2}s",
        num_proofs,
        individual_prove_duration.as_secs_f64()
    );

    // Now aggregate the proofs
    println!("\nAggregating proofs...");
    let aggregate_start = Instant::now();

    let mut aggregate_stdin = SP1Stdin::new();

    // Write verification keys
    let vkeys: Vec<[u32; 8]> = proofs.iter().map(|(_, vk)| vk.hash_u32()).collect();
    aggregate_stdin.write::<Vec<[u32; 8]>>(&vkeys);

    // Write public values
    let public_values: Vec<Vec<u8>> = proofs
        .iter()
        .map(|(proof, _)| proof.public_values.to_vec())
        .collect();
    aggregate_stdin.write::<Vec<Vec<u8>>>(&public_values);

    // Write the proofs for verification
    for (proof, vk) in &proofs {
        let SP1Proof::Compressed(compressed_proof) = &proof.proof else {
            panic!("Expected compressed proof for aggregation")
        };
        aggregate_stdin.write_proof(*compressed_proof.clone(), vk.vk.clone());
    }

    // Ensure circuit artifacts are available before final proof generation
    ensure_circuit_artifacts(final_proof_type)?;

    // Generate the final proof based on the selected proof type
    // Note: When using ProverClient::from_env(), the strategy is automatically determined
    // based on the SP1_PROVER environment variable we set earlier
    let final_proof = match final_proof_type {
        FinalProofType::Core => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .compressed()
            .run()?,
        FinalProofType::Groth16 => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .groth16()
            .run()?,
        FinalProofType::Plonk => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .plonk()
            .run()?,
    };

    let aggregate_duration = aggregate_start.elapsed();
    println!(
        "Aggregation completed in {:.2}s",
        aggregate_duration.as_secs_f64()
    );

    // Verify the aggregated proof
    println!("\nVerifying aggregated proof...");
    let verify_start = Instant::now();
    aggregate_client.verify(&final_proof, &aggregate_vk)?;
    let verify_duration = verify_start.elapsed();
    println!(
        "Verification completed in {:.2}s",
        verify_duration.as_secs_f64()
    );

    // Decode aggregated public values
    let aggregated_values = PublicValuesStruct::abi_decode(final_proof.public_values.as_slice())
        .expect("failed to decode aggregated public values");

    // Verify the aggregated values match first and last roots
    let first_proof_values = PublicValuesStruct::abi_decode(proofs[0].0.public_values.as_slice())
        .expect("failed to decode first proof");
    let last_proof_values =
        PublicValuesStruct::abi_decode(proofs[proofs.len() - 1].0.public_values.as_slice())
            .expect("failed to decode last proof");

    assert_eq!(
        aggregated_values.old_root, first_proof_values.old_root,
        "Aggregated old_root mismatch"
    );
    assert_eq!(
        aggregated_values.new_root, last_proof_values.new_root,
        "Aggregated new_root mismatch"
    );

    Ok(AggregationResult {
        proof: final_proof,
        setup_duration,
        individual_prove_duration,
        aggregate_duration,
        verify_duration,
        aggregated_values,
        aggregate_vk,
    })
}

/// Generate a single proof without aggregation
pub fn generate_single_proof(
    elf: &[u8],
    stdin: &SP1Stdin,
    final_proof_type: FinalProofType,
) -> Result<SP1ProofWithPublicValues, Box<dyn std::error::Error>> {
    // Ensure circuit artifacts are available before proof generation
    println!("Checking circuit artifacts...");
    ensure_circuit_artifacts(final_proof_type).map_err(|e| {
        eprintln!("❌ Circuit artifacts check failed: {}", e);
        e
    })?;

    // Create prover client
    println!("Initializing prover client...");
    let client = ProverClient::builder().cpu().build();

    println!("Setting up proving/verifying keys...");
    let (pk, _vk) = client.setup(elf);

    let prove_start = Instant::now();

    println!("\nGenerating SP1 proof...");
    println!("  ⏳ This may take a while for large inputs...");

    let proof = match final_proof_type {
        FinalProofType::Core => {
            println!("  Proof type: Core (compressed)");
            client.prove(&pk, stdin).compressed().run()
                .map_err(|e| {
                    eprintln!("\n❌ Core proof generation failed!");
                    eprintln!("Error: {}", e);
                    e
                })?
        }
        FinalProofType::Groth16 => {
            println!("  Proof type: Groth16");
            println!("  📊 Stages: core → compress → shrink → wrap → groth16");
            client.prove(&pk, stdin).groth16().run()
                .map_err(|e| {
                    eprintln!("\n❌ Groth16 proof generation failed!");
                    eprintln!("Error: {}", e);
                    eprintln!("\nPossible causes:");
                    eprintln!("  - Memory exhaustion (try reducing --count)");
                    eprintln!("  - Circuit artifacts missing or corrupted");
                    eprintln!("  - Docker-in-Docker issues (gnark container failed)");
                    eprintln!("  - Input data too large for proof system");
                    e
                })?
        }
        FinalProofType::Plonk => {
            println!("  Proof type: PLONK");
            println!("  📊 Stages: core → compress → shrink → wrap → plonk");
            client.prove(&pk, stdin).plonk().run()
                .map_err(|e| {
                    eprintln!("\n❌ PLONK proof generation failed!");
                    eprintln!("Error: {}", e);
                    eprintln!("\nPossible causes:");
                    eprintln!("  - Memory exhaustion (try reducing --count)");
                    eprintln!("  - Circuit artifacts missing or corrupted");
                    eprintln!("  - Docker-in-Docker issues (gnark container failed)");
                    e
                })?
        }
    };

    let prove_duration = prove_start.elapsed();
    println!("✓ Proof generated in {:.2}s", prove_duration.as_secs_f64());

    Ok(proof)
}

/// Verify a proof and return the verification duration
pub fn verify_proof(
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    println!("\nVerifying proof...");
    let verify_start = Instant::now();

    // Create prover client for verification
    let client = ProverClient::builder().cpu().build();
    client.verify(proof, vk)?;

    let verify_duration = verify_start.elapsed();
    println!("✅ Proof verified successfully!");

    Ok(verify_duration)
}

/// Print proof statistics
pub fn print_proof_statistics(proof: &SP1ProofWithPublicValues) {
    println!("\n=== Proof Statistics ===");

    match &proof.proof {
        SP1Proof::Core(_) => println!("Proof type: Core"),
        SP1Proof::Compressed(_) => println!("Proof type: Compressed"),
        SP1Proof::Plonk(_) => println!("Proof type: PLONK"),
        SP1Proof::Groth16(_) => println!("Proof type: Groth16"),
    }

    println!(
        "Public values size: {} bytes",
        proof.public_values.as_slice().len()
    );

    // Decode and print the public values
    let decoded = PublicValuesStruct::abi_decode(proof.public_values.as_slice())
        .expect("failed to decode public values");

    println!(
        "Old root: 0x{}",
        hex::encode(decoded.old_root.to_be_bytes::<32>())
    );
    println!(
        "New root: 0x{}",
        hex::encode(decoded.new_root.to_be_bytes::<32>())
    );
}

/// Print aggregated results
pub fn print_aggregated_results(values: &PublicValuesStruct) {
    println!("\n=== Aggregated Results ===");
    println!(
        "First old_root: 0x{}",
        hex::encode(values.old_root.to_be_bytes::<32>())
    );
    println!(
        "Final new_root: 0x{}",
        hex::encode(values.new_root.to_be_bytes::<32>())
    );
}

/// Print aggregation performance summary
pub fn print_aggregation_performance_summary(result: &AggregationResult) {
    println!("\n=== Performance Summary ===");
    println!(
        "Setup time:           {:.2}s",
        result.setup_duration.as_secs_f64()
    );
    println!(
        "Individual proofs:    {:.2}s",
        result.individual_prove_duration.as_secs_f64()
    );
    println!(
        "Aggregation:          {:.2}s",
        result.aggregate_duration.as_secs_f64()
    );
    println!(
        "Verification:         {:.2}s",
        result.verify_duration.as_secs_f64()
    );
    println!(
        "Total time:           {:.2}s",
        result.setup_duration.as_secs_f64()
            + result.individual_prove_duration.as_secs_f64()
            + result.aggregate_duration.as_secs_f64()
            + result.verify_duration.as_secs_f64()
    );
}

/// Save core proof to JSON file
pub fn save_core_proof(
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Deserialize the public values
    let bytes = proof.public_values.as_slice();
    let public_values = PublicValuesStruct::abi_decode(bytes)?;

    // Serialize the proof using bincode for all proof types
    let proof_bytes = bincode::serialize(proof)?;
    
    // Serialize the verifying key for actual verification
    let vkey_bytes = bincode::serialize(vk)?;
    
    // Print proof size information
    println!("\n📊 Proof Size Information:");
    println!("   Serialized proof size: {} bytes ({:.2} KB)", 
        proof_bytes.len(), 
        proof_bytes.len() as f64 / 1024.0
    );
    println!("   Verifying key size: {} bytes ({:.2} KB)", 
        vkey_bytes.len(), 
        vkey_bytes.len() as f64 / 1024.0
    );
    println!("   Public values size: {} bytes", bytes.len());
    
    // Create the fixture
    let fixture = CoreProofFixture {
        old_root: format!(
            "0x{}",
            hex::encode(public_values.old_root.to_be_bytes::<32>())
        ),
        new_root: format!(
            "0x{}",
            hex::encode(public_values.new_root.to_be_bytes::<32>())
        ),
        vkey: vk.bytes32().to_string(),
        vkey_bytes: Some(format!("0x{}", hex::encode(vkey_bytes))),
        public_values: format!("0x{}", hex::encode(bytes)),
        proof: format!("0x{}", hex::encode(proof_bytes)),
    };

    // Create proofs directory if it doesn't exist
    let proof_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");
    std::fs::create_dir_all(&proof_dir)?;

    // Generate filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let filename = format!("core-proof-{}.json", timestamp);
    let filepath = proof_dir.join(&filename);

    // Write to file
    let json_content = serde_json::to_string_pretty(&fixture)?;
    std::fs::write(&filepath, &json_content)?;
    
    // Print save confirmation with file size
    println!("\n✅ Core proof saved to: proofs/{}", filename);
    println!("   File size: {} bytes ({:.2} KB)", 
        json_content.len(), 
        json_content.len() as f64 / 1024.0
    );
    println!("   Verification key: {}", fixture.vkey);

    Ok(filepath)
}

/// Load core proof from JSON file
pub fn load_core_proof(
    filepath: &PathBuf,
) -> Result<(CoreProofFixture, SP1ProofWithPublicValues), Box<dyn std::error::Error>> {
    let json_str = std::fs::read_to_string(filepath)?;
    let fixture: CoreProofFixture = serde_json::from_str(&json_str)?;

    // Convert the proof back to SP1ProofWithPublicValues
    let proof_bytes = hex::decode(fixture.proof.trim_start_matches("0x"))?;
    let _public_values_bytes = hex::decode(fixture.public_values.trim_start_matches("0x"))?;

    // Reconstruct SP1ProofWithPublicValues using bincode deserialization
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&proof_bytes)?;

    Ok((fixture, proof))
}

// ============================================================================
// Shrink Proof Generation (zkVerify Compatible)
// ============================================================================

/// Shrink proof fixture for zkVerify compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShrinkProofFixture {
    pub old_root: String,
    pub new_root: String,
    pub vkey_hash: String,  // For zkVerify (vk.hash_bytes())
    pub vkey_bytes: String, // Serialized SP1VerifyingKey for verification
    pub public_values: String,
    pub shrink_proof: String, // Raw SP1ReduceProof serialized
}

/// Result of shrink proof aggregation
pub struct ShrinkAggregationResult {
    pub shrink_proof: SP1ReduceProof<InnerSC>,
    pub public_values: SP1PublicValues,
    pub setup_duration: std::time::Duration,
    pub individual_prove_duration: std::time::Duration,
    pub aggregate_duration: std::time::Duration,
    pub shrink_duration: std::time::Duration,
    pub verify_duration: std::time::Duration,
    pub aggregated_values: PublicValuesStruct,
    pub aggregate_vk: SP1VerifyingKey,
}

/// Generate a single shrink proof using lower-level SP1 Prover API
pub fn generate_single_shrink_proof(
    elf: &[u8],
    stdin: &SP1Stdin,
) -> Result<(SP1ReduceProof<InnerSC>, SP1PublicValues), Box<dyn std::error::Error>> {
    println!("\nGenerating SP1 shrink proof (zkVerify compatible)...");

    // Create CPU prover with access to lower-level API
    let client = ProverClient::builder().cpu().build();
    let (pk, vk) = client.setup(elf);

    // Access the inner SP1Prover to use lower-level API
    let prover: &SP1Prover<CpuProverComponents> = client.inner();
    let opts = SP1ProverOpts::default();
    let context = SP1Context::default();

    let prove_start = Instant::now();

    // Get the program from ELF
    let program = prover.get_program(elf).unwrap();

    // Step 1: Execute and generate core proof
    println!("  Step 1/3: Generating core proof...");
    let core_proof = prover.prove_core(&pk.pk, program, stdin, opts, context)?;
    let public_values = core_proof.public_values.clone();

    // Step 2: Compress to single proof
    println!("  Step 2/3: Compressing proof...");
    let deferred_proofs = stdin.proofs.iter().map(|(reduce_proof, _)| reduce_proof.clone()).collect();
    let compressed_proof = prover.compress(&vk, core_proof, deferred_proofs, opts)?;

    // Step 3: Shrink to SNARK-friendly field (THIS IS WHAT zkVerify NEEDS!)
    println!("  Step 3/3: Shrinking to SNARK-friendly field...");
    let shrink_proof = prover.shrink(compressed_proof, opts)?;

    let prove_duration = prove_start.elapsed();
    println!("Shrink proof generated in {:.2}s", prove_duration.as_secs_f64());

    Ok((shrink_proof, public_values))
}

/// Generate multiple shrink proofs and aggregate them
pub fn generate_and_aggregate_shrink_proofs(
    add_elf: &[u8],
    aggregate_elf: &[u8],
    num_proofs: u32,
    operations_per_proof: u32,
    _use_hosted: bool,
) -> Result<ShrinkAggregationResult, Box<dyn std::error::Error>> {
    println!("Generating {} shrink proofs for aggregation...", num_proofs);

    // Setup proving keys for both add and aggregate programs
    println!("Setting up proving keys...");
    let setup_start = Instant::now();

    // Shrink proofs always need CPU prover for the low-level .shrink() API
    // Using from_env() adds significant overhead when calling .inner()
    // Even if aggregation used network, we'd still need to download and shrink on CPU
    // Better to do entire pipeline on CPU for consistency and performance
    let add_client = ProverClient::builder().cpu().build();
    let aggregate_client = ProverClient::builder().cpu().build();
    let (add_pk, add_vk) = add_client.setup(add_elf);
    let (aggregate_pk, aggregate_vk) = aggregate_client.setup(aggregate_elf);
    let setup_duration = setup_start.elapsed();
    println!("Setup completed in {:.2}s", setup_duration.as_secs_f64());

    // Initialize account manager
    let mut manager = AccountManager::new(16);

    // Generate multiple proofs with chained roots
    let mut proofs = Vec::new();
    let prove_start = Instant::now();
    let mut rng = thread_rng();

    for i in 0..num_proofs {
        println!("\nGenerating proof {} of {}...", i + 1, num_proofs);

        // Get current root before operations
        let proof_initial_root = manager.get_root();

        // Generate random account operations for this proof
        let operations: Vec<AccountOperation> = (0..operations_per_proof)
            .map(|_| {
                let account_num = rng.gen_range(1..=100);
                let add_value = rng.gen_range(1..=1000);
                AccountOperation::new(account_num, add_value)
            })
            .collect();

        println!("  Processing {} operations", operations.len());

        // Setup inputs for this proof
        let mut stdin = SP1Stdin::new();

        // Write initial root
        let root_bytes: [u8; 32] = proof_initial_root.as_bytes().try_into().unwrap();
        stdin.write(&root_bytes);

        // Write number of operations
        stdin.write(&operations_per_proof);

        // Process operations and write actions
        for op in &operations {
            let action = manager
                .process_action(op.account_num, op.add_value)
                .expect("Failed to process action");

            // Serialize the action for zkVM
            let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
            stdin.write(&action_bytes);
        }

        let proof_final_root = manager.get_root();
        println!(
            "  Old root: 0x{}",
            hex::encode(proof_initial_root.as_bytes())
        );
        println!("  New root: 0x{}", hex::encode(proof_final_root.as_bytes()));

        // Generate compressed proof for aggregation
        let proof = add_client
            .prove(&add_pk, &stdin)
            .compressed()
            .run()
            .expect("failed to generate proof");

        // Verify the proof output matches expected
        let decoded = PublicValuesStruct::abi_decode(proof.public_values.as_slice())
            .expect("failed to decode public values");

        let old_bytes: [u8; 32] = proof_initial_root.as_bytes().try_into().unwrap();
        let expected_old = U256::from_be_bytes(old_bytes);
        let new_bytes: [u8; 32] = proof_final_root.as_bytes().try_into().unwrap();
        let expected_new = U256::from_be_bytes(new_bytes);

        assert_eq!(
            decoded.old_root, expected_old,
            "Old root mismatch in proof {}",
            i
        );
        assert_eq!(
            decoded.new_root, expected_new,
            "New root mismatch in proof {}",
            i
        );

        proofs.push((proof, add_vk.clone()));
    }

    let individual_prove_duration = prove_start.elapsed();
    println!(
        "\nGenerated {} individual proofs in {:.2}s",
        num_proofs,
        individual_prove_duration.as_secs_f64()
    );

    // Now aggregate the proofs
    println!("\nAggregating proofs...");
    let aggregate_start = Instant::now();

    let mut aggregate_stdin = SP1Stdin::new();

    // Write verification keys
    let vkeys: Vec<[u32; 8]> = proofs.iter().map(|(_, vk)| vk.hash_u32()).collect();
    aggregate_stdin.write::<Vec<[u32; 8]>>(&vkeys);

    // Write public values
    let public_values: Vec<Vec<u8>> = proofs
        .iter()
        .map(|(proof, _)| proof.public_values.to_vec())
        .collect();
    aggregate_stdin.write::<Vec<Vec<u8>>>(&public_values);

    // Write the proofs for verification
    for (proof, vk) in &proofs {
        let SP1Proof::Compressed(compressed_proof) = &proof.proof else {
            panic!("Expected compressed proof for aggregation")
        };
        aggregate_stdin.write_proof(*compressed_proof.clone(), vk.vk.clone());
    }

    // Generate aggregated compressed proof
    let aggregated_proof = aggregate_client
        .prove(&aggregate_pk, &aggregate_stdin)
        .compressed()
        .run()?;

    let aggregate_duration = aggregate_start.elapsed();
    println!(
        "Aggregation completed in {:.2}s",
        aggregate_duration.as_secs_f64()
    );

    // Now shrink the aggregated proof for zkVerify
    println!("\nShrinking aggregated proof for zkVerify...");
    let shrink_start = Instant::now();
    let agg_prover: &SP1Prover<CpuProverComponents> = aggregate_client.inner();

    // Extract the compressed proof
    let SP1Proof::Compressed(compressed_agg) = &aggregated_proof.proof else {
        return Err("Expected compressed proof".into());
    };

    // Shrink it
    let shrink_proof = agg_prover.shrink(*compressed_agg.clone(), SP1ProverOpts::default())?;

    let shrink_duration = shrink_start.elapsed();
    println!(
        "Shrinking completed in {:.2}s",
        shrink_duration.as_secs_f64()
    );

    // Verify the shrink proof
    println!("\nVerifying shrink proof...");
    let verify_start = Instant::now();
    // Note: We can't verify shrink proofs directly with the SDK's verify method
    // The verification would happen on zkVerify
    let verify_duration = verify_start.elapsed();
    println!(
        "Shrink proof prepared for zkVerify verification in {:.2}s",
        verify_duration.as_secs_f64()
    );

    // Decode aggregated public values
    let aggregated_values = PublicValuesStruct::abi_decode(aggregated_proof.public_values.as_slice())
        .expect("failed to decode aggregated public values");

    // Verify the aggregated values match first and last roots
    let first_proof_values = PublicValuesStruct::abi_decode(proofs[0].0.public_values.as_slice())
        .expect("failed to decode first proof");
    let last_proof_values =
        PublicValuesStruct::abi_decode(proofs[proofs.len() - 1].0.public_values.as_slice())
            .expect("failed to decode last proof");

    assert_eq!(
        aggregated_values.old_root, first_proof_values.old_root,
        "Aggregated old_root mismatch"
    );
    assert_eq!(
        aggregated_values.new_root, last_proof_values.new_root,
        "Aggregated new_root mismatch"
    );

    Ok(ShrinkAggregationResult {
        shrink_proof,
        public_values: aggregated_proof.public_values,
        setup_duration,
        individual_prove_duration,
        aggregate_duration,
        shrink_duration,
        verify_duration,
        aggregated_values,
        aggregate_vk,
    })
}

/// Verify a shrink proof
pub fn verify_shrink_proof(
    _shrink_proof: &SP1ReduceProof<InnerSC>,
    _vk: &SP1VerifyingKey,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    println!("\n⚠️  Note: Shrink proofs are designed for zkVerify verification");
    println!("   Local verification is not available through the standard SDK API");
    println!("   The proof will be verified when submitted to zkVerify");

    let verify_start = Instant::now();
    // Shrink proofs can't be verified directly with the standard SDK verify method
    // They need to be verified on zkVerify
    let verify_duration = verify_start.elapsed();

    Ok(verify_duration)
}

/// Print shrink proof statistics
pub fn print_shrink_proof_statistics(shrink_proof: &SP1ReduceProof<InnerSC>) {
    println!("\n=== Shrink Proof Statistics ===");
    println!("Proof type: Shrink (SNARK-friendly field STARK)");

    // Serialize to get size
    let proof_bytes = bincode::serialize(shrink_proof).unwrap_or_default();
    println!(
        "Proof size: {} bytes ({:.2} KB)",
        proof_bytes.len(),
        proof_bytes.len() as f64 / 1024.0
    );
}

/// Save shrink proof to JSON file
pub fn save_shrink_proof(
    shrink_proof: &SP1ReduceProof<InnerSC>,
    vk: &SP1VerifyingKey,
    public_values: &SP1PublicValues,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Deserialize the public values
    let bytes = public_values.as_slice();
    let decoded_values = PublicValuesStruct::abi_decode(bytes)?;

    // Serialize the shrink proof
    let proof_bytes = bincode::serialize(shrink_proof)?;

    // Serialize the verifying key
    let vkey_bytes = bincode::serialize(vk)?;

    // Get vkey hash for zkVerify
    let vkey_hash = vk.hash_bytes();

    // Print proof size information
    println!("\n📊 Shrink Proof Size Information:");
    println!("   Serialized proof size: {} bytes ({:.2} KB)",
        proof_bytes.len(),
        proof_bytes.len() as f64 / 1024.0
    );
    println!("   Verifying key size: {} bytes ({:.2} KB)",
        vkey_bytes.len(),
        vkey_bytes.len() as f64 / 1024.0
    );
    println!("   Public values size: {} bytes", bytes.len());

    // Create the fixture
    let fixture = ShrinkProofFixture {
        old_root: format!(
            "0x{}",
            hex::encode(decoded_values.old_root.to_be_bytes::<32>())
        ),
        new_root: format!(
            "0x{}",
            hex::encode(decoded_values.new_root.to_be_bytes::<32>())
        ),
        vkey_hash: format!("0x{}", hex::encode(vkey_hash)),
        vkey_bytes: format!("0x{}", hex::encode(vkey_bytes)),
        public_values: format!("0x{}", hex::encode(bytes)),
        shrink_proof: format!("0x{}", hex::encode(proof_bytes)),
    };

    // Create proofs directory if it doesn't exist
    let proof_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");
    std::fs::create_dir_all(&proof_dir)?;

    // Generate filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let filename = format!("shrink-proof-{}.json", timestamp);
    let filepath = proof_dir.join(&filename);

    // Write to file
    let json_content = serde_json::to_string_pretty(&fixture)?;
    std::fs::write(&filepath, &json_content)?;

    // Print save confirmation with file size
    println!("\n✅ Shrink proof saved to: proofs/{}", filename);
    println!("   File size: {} bytes ({:.2} KB)",
        json_content.len(),
        json_content.len() as f64 / 1024.0
    );
    println!("   Verification key hash (for zkVerify): {}", fixture.vkey_hash);
    println!("\n💡 This proof is ready for zkVerify submission!");

    Ok(filepath)
}

/// Print aggregation performance summary for shrink proofs
pub fn print_aggregation_performance_summary_shrink(result: &ShrinkAggregationResult) {
    println!("\n=== Performance Summary ===");
    println!(
        "Setup time:           {:.2}s",
        result.setup_duration.as_secs_f64()
    );
    println!(
        "Individual proofs:    {:.2}s",
        result.individual_prove_duration.as_secs_f64()
    );
    println!(
        "Aggregation:          {:.2}s",
        result.aggregate_duration.as_secs_f64()
    );
    println!(
        "Shrink:               {:.2}s",
        result.shrink_duration.as_secs_f64()
    );
    println!(
        "Verification prep:    {:.2}s",
        result.verify_duration.as_secs_f64()
    );
    println!(
        "Total time:           {:.2}s",
        result.setup_duration.as_secs_f64()
            + result.individual_prove_duration.as_secs_f64()
            + result.aggregate_duration.as_secs_f64()
            + result.shrink_duration.as_secs_f64()
            + result.verify_duration.as_secs_f64()
    );
}

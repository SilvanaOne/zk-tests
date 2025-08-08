//! Shared proof generation and aggregation utilities

use add_lib::PublicValuesStruct;
use crate::map::{AccountManager, AccountOperation};
use alloy_sol_types::{SolType, private::U256};
use sp1_sdk::{
    HashableKey, Prover, ProverClient, SP1Proof, SP1ProofWithPublicValues,
    SP1Stdin, SP1VerifyingKey, network::FulfillmentStrategy,
};
use std::time::Instant;
use rand::{thread_rng, Rng};
use hex;

/// Enum representing the available proof types for final proof generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalProofType {
    Core,
    Groth16,
    Plonk,
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
) -> Result<AggregationResult, Box<dyn std::error::Error>> {
    println!("Generating {} proofs for aggregation...", num_proofs);
    
    // Setup proving keys for both add and aggregate programs
    println!("Setting up proving keys...");
    let setup_start = Instant::now();
    let add_client = ProverClient::builder().cpu().build();
    let aggregate_client = ProverClient::builder().network().build();
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
            let action = manager.process_action(op.account_num, op.add_value)
                .expect("Failed to process action");
            
            // Serialize the action for zkVM
            let action_bytes = borsh::to_vec(&action).expect("Failed to serialize action");
            stdin.write(&action_bytes);
        }
        
        let proof_final_root = manager.get_root();
        println!("  Old root: 0x{}", hex::encode(proof_initial_root.as_bytes()));
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
        
        assert_eq!(decoded.old_root, expected_old, "Old root mismatch in proof {}", i);
        assert_eq!(decoded.new_root, expected_new, "New root mismatch in proof {}", i);
        
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
    
    // Generate the final proof based on the selected proof type
    let final_proof = match final_proof_type {
        FinalProofType::Core => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .compressed()
            .strategy(FulfillmentStrategy::Hosted)
            .run()?,
        FinalProofType::Groth16 => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .groth16()
            .strategy(FulfillmentStrategy::Hosted)
            .run()?,
        FinalProofType::Plonk => aggregate_client
            .prove(&aggregate_pk, &aggregate_stdin)
            .plonk()
            .strategy(FulfillmentStrategy::Hosted)
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
    let last_proof_values = PublicValuesStruct::abi_decode(proofs[proofs.len() - 1].0.public_values.as_slice())
        .expect("failed to decode last proof");
    
    assert_eq!(aggregated_values.old_root, first_proof_values.old_root, "Aggregated old_root mismatch");
    assert_eq!(aggregated_values.new_root, last_proof_values.new_root, "Aggregated new_root mismatch");
    
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
    // Create prover client
    let client = ProverClient::builder().cpu().build();
    let (pk, _vk) = client.setup(elf);
    
    let prove_start = Instant::now();
    
    println!("\nGenerating SP1 proof...");
    
    let proof = match final_proof_type {
        FinalProofType::Core => {
            println!("  Proof type: Core (compressed)");
            client.prove(&pk, stdin).compressed().run()?
        }
        FinalProofType::Groth16 => {
            println!("  Proof type: Groth16");
            client.prove(&pk, stdin).groth16().run()?
        }
        FinalProofType::Plonk => {
            println!("  Proof type: PLONK");
            client.prove(&pk, stdin).plonk().run()?
        }
    };
    
    let prove_duration = prove_start.elapsed();
    println!("Proof generated in {:.2}s", prove_duration.as_secs_f64());
    
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
    
    println!("Public values size: {} bytes", proof.public_values.as_slice().len());
    
    // Decode and print the public values
    let decoded = PublicValuesStruct::abi_decode(proof.public_values.as_slice())
        .expect("failed to decode public values");
    
    println!("Old root: 0x{}", hex::encode(decoded.old_root.to_be_bytes::<32>()));
    println!("New root: 0x{}", hex::encode(decoded.new_root.to_be_bytes::<32>()));
}

/// Print aggregated results
pub fn print_aggregated_results(values: &PublicValuesStruct) {
    println!("\n=== Aggregated Results ===");
    println!("First old_root: 0x{}", hex::encode(values.old_root.to_be_bytes::<32>()));
    println!("Final new_root: 0x{}", hex::encode(values.new_root.to_be_bytes::<32>()));
}

/// Print aggregation performance summary
pub fn print_aggregation_performance_summary(result: &AggregationResult) {
    println!("\n=== Performance Summary ===");
    println!("Setup time:           {:.2}s", result.setup_duration.as_secs_f64());
    println!("Individual proofs:    {:.2}s", result.individual_prove_duration.as_secs_f64());
    println!("Aggregation:          {:.2}s", result.aggregate_duration.as_secs_f64());
    println!("Verification:         {:.2}s", result.verify_duration.as_secs_f64());
    println!(
        "Total time:           {:.2}s",
        result.setup_duration.as_secs_f64()
            + result.individual_prove_duration.as_secs_f64()
            + result.aggregate_duration.as_secs_f64()
            + result.verify_duration.as_secs_f64()
    );
}
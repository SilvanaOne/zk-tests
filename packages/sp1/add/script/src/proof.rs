//! Shared proof generation and aggregation utilities

use add_lib::PublicValuesStruct;
use alloy_sol_types::SolType;
use sp1_sdk::{
    EnvProver, HashableKey, Prover, ProverClient, SP1Proof, SP1ProofWithPublicValues,
    SP1ProvingKey, SP1Stdin, SP1VerifyingKey, network::FulfillmentStrategy,
};
use std::time::Instant;

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
    pub individual_prove_duration: std::time::Duration,
    pub aggregate_duration: std::time::Duration,
    pub verify_duration: std::time::Duration,
    pub aggregated_values: PublicValuesStruct,
    pub aggregate_vk: SP1VerifyingKey,
}

/// Generate multiple proofs and aggregate them
pub fn generate_and_aggregate_proofs(
    add_elf: &[u8],
    aggregate_elf: &[u8],
    num_proofs: u32,
    length: u32,
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

    // Generate multiple proofs with chained old_sum/new_sum
    let mut proofs = Vec::new();
    let mut current_old_sum = 0u32;
    let prove_start = Instant::now();

    for i in 0..num_proofs {
        println!("\nGenerating proof {} of {}...", i + 1, num_proofs);

        // Setup inputs for this proof
        let mut stdin = SP1Stdin::new();
        let values: Vec<u32> = (10..10 + length).collect();

        stdin.write(&current_old_sum);
        stdin.write(&length);
        for &value in &values {
            stdin.write(&value);
        }

        println!("  old_sum: {current_old_sum}");
        println!("  values: {values:?}");

        // Generate compressed proof for aggregation
        let proof = add_client
            .prove(&add_pk, &stdin)
            .compressed()
            .run()
            .expect("failed to generate proof");

        // Decode public values to get new_sum for next proof
        let decoded = PublicValuesStruct::abi_decode(proof.public_values.as_slice())
            .expect("failed to decode public values");
        println!("  new_sum: {}", decoded.new_sum);

        // Update old_sum for next proof
        current_old_sum = decoded.new_sum;
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
    aggregate_client
        .verify(&final_proof, &aggregate_vk)
        .expect("failed to verify aggregated proof");
    let verify_duration = verify_start.elapsed();

    println!("Successfully verified aggregated proof!");
    println!("Verification time: {:.2}s", verify_duration.as_secs_f64());

    // Get aggregated public values
    let aggregated_values = PublicValuesStruct::abi_decode(final_proof.public_values.as_slice())
        .expect("failed to decode aggregated public values");

    Ok(AggregationResult {
        proof: final_proof,
        individual_prove_duration,
        aggregate_duration,
        verify_duration,
        aggregated_values,
        aggregate_vk,
    })
}

/// Print detailed proof statistics
pub fn print_proof_statistics(proof: &SP1ProofWithPublicValues) {
    println!("\n=== SP1 Proof Statistics ===");
    println!("Proof type: {}", proof.proof);
    println!("SP1 version: {}", proof.sp1_version);

    // Get proof size information using SP1's standard approach
    let proof_bytes = bincode::serialize(&proof).expect("failed to serialize proof");
    let proof_size_bytes = proof_bytes.len();
    let proof_size_kb = proof_size_bytes as f64 / 1024.0;
    let proof_size_mb = proof_size_kb / 1024.0;

    println!("Proof size: {proof_size_bytes} bytes ({proof_size_kb:.2} KB, {proof_size_mb:.3} MB)");

    match &proof.proof {
        SP1Proof::Core(core_proofs) => {
            println!("Number of shards: {}", core_proofs.len());
            if !core_proofs.is_empty() {
                let avg_shard_size = proof_size_bytes / core_proofs.len();
                println!(
                    "Average shard size: {} bytes ({:.2} KB)",
                    avg_shard_size,
                    avg_shard_size as f64 / 1024.0
                );
            }
        }
        SP1Proof::Compressed(_) => {
            println!("Compressed proof (constant size)");
        }
        SP1Proof::Groth16(_) => {
            println!("Groth16 proof (constant size)");
        }
        SP1Proof::Plonk(_) => {
            println!("PLONK proof (constant size)");
        }
    }

    // Print public values info
    let public_values_bytes = proof.public_values.as_slice();
    println!("Public values size: {} bytes", public_values_bytes.len());
    println!(
        "Public values (hex): 0x{}",
        hex::encode(public_values_bytes)
    );

    // Optionally save proof to file using SP1's built-in save method
    if std::env::var("SP1_SAVE_PROOF").is_ok() {
        let proof_path = "../proofs/proof.bin";
        println!("Saving proof to: {proof_path}");
        if let Err(e) = proof.save(proof_path) {
            println!("Warning: Failed to save proof: {e}");
        } else {
            println!("Proof saved successfully");
        }
    }
}

/// Print performance summary for aggregation
pub fn print_aggregation_performance_summary(
    setup_duration: std::time::Duration,
    result: &AggregationResult,
) {
    println!("\n=== Performance Summary ===");
    println!("Setup time:        {:.2}s", setup_duration.as_secs_f64());
    println!(
        "Individual proofs: {:.2}s",
        result.individual_prove_duration.as_secs_f64()
    );
    println!(
        "Aggregation:       {:.2}s",
        result.aggregate_duration.as_secs_f64()
    );
    println!(
        "Verification:      {:.2}s",
        result.verify_duration.as_secs_f64()
    );
    println!(
        "Total time:        {:.2}s",
        (setup_duration
            + result.individual_prove_duration
            + result.aggregate_duration
            + result.verify_duration)
            .as_secs_f64()
    );
}

/// Print aggregated results
pub fn print_aggregated_results(values: &PublicValuesStruct) {
    println!("\n=== Aggregated Results ===");
    println!("First old_sum: {}", values.old_sum);
    println!("Final new_sum: {}", values.new_sum);
    println!("Total added: {}", values.new_sum - values.old_sum);
}

/// Generate a single proof without aggregation
pub fn generate_single_proof(
    client: &EnvProver,
    pk: &SP1ProvingKey,
    stdin: &SP1Stdin,
    proof_type: FinalProofType,
) -> Result<SP1ProofWithPublicValues, Box<dyn std::error::Error>> {
    println!(
        "\nGenerating {} proof...",
        format!("{:?}", proof_type).to_lowercase()
    );
    let prove_start = Instant::now();

    let proof = match proof_type {
        FinalProofType::Core => client.prove(pk, stdin).core().run()?,
        FinalProofType::Groth16 => client.prove(pk, stdin).groth16().run()?,
        FinalProofType::Plonk => client.prove(pk, stdin).plonk().run()?,
    };

    let prove_duration = prove_start.elapsed();
    println!(
        "Successfully generated {} proof!",
        format!("{:?}", proof_type).to_lowercase()
    );
    println!("Proving time: {:.2}s", prove_duration.as_secs_f64());

    Ok(proof)
}

/// Verify a proof
pub fn verify_proof(
    client: &EnvProver,
    proof: &SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
    println!("\nVerifying proof...");
    let verify_start = Instant::now();
    client.verify(proof, vk)?;
    let verify_duration = verify_start.elapsed();

    println!("Successfully verified proof!");
    println!("Verification time: {:.2}s", verify_duration.as_secs_f64());

    Ok(verify_duration)
}

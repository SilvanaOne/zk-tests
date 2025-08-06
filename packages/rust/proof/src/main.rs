use mina_hasher::Fp;
use std::time::Instant;

mod poseidon;
mod proof;

use poseidon::poseidon_hash;
use proof::{create_poseidon_proof, get_circuit_info, verify_poseidon_proof};

fn main() {
    println!("=== Poseidon Hash ZK Proof Demo ===\n");

    // Step 1: Calculate the Poseidon hash of [1, 2, 3]
    println!("Step 1: Poseidon hash of [1, 2, 3]:");
    let values = vec![Fp::from(1u64), Fp::from(2u64), Fp::from(3u64)];

    let hash_start = Instant::now();
    let hash = poseidon_hash(&values);
    let hash_duration = hash_start.elapsed();

    println!("  Hash: {}", hash);
    println!("  Time: {:.3} ms", hash_duration.as_secs_f64() * 1000.0);
    println!();

    // Step 2: Display circuit information
    println!("Step 2: Circuit information:");
    let (gates_count, constraints_info) = get_circuit_info();
    println!("  Number of gates: {}", gates_count);
    println!("  {}", constraints_info);
    println!();

    // Step 3: Create a zero-knowledge proof
    println!("Step 3: Creating ZK proof that we know the preimage...");
    let proof_start = Instant::now();
    match create_poseidon_proof() {
        Ok(proof_data) => {
            let proof_duration = proof_start.elapsed();
            println!("  ✓ Proof created successfully!");
            println!("  Time: {:.3} ms", proof_duration.as_secs_f64() * 1000.0);

            // Step 4: Verify the proof
            println!("\nStep 4: Verifying the proof...");
            let verify_start = Instant::now();
            match verify_poseidon_proof(&proof_data) {
                Ok(is_valid) => {
                    let verify_duration = verify_start.elapsed();
                    if is_valid {
                        println!("  ✓ Proof verified successfully!");
                        println!("  Time: {:.3} ms", verify_duration.as_secs_f64() * 1000.0);
                    } else {
                        println!("  ✗ Proof verification failed!");
                    }
                }
                Err(e) => println!("  ✗ Error verifying proof: {}", e),
            }
        }
        Err(e) => println!("  ✗ Error creating proof: {}", e),
    }
}

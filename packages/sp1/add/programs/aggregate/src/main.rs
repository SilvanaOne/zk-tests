//! A program that aggregates multiple SP1 add proofs.
//! This program verifies multiple add program proofs and ensures they form a valid chain
//! where each proof's old_sum matches the previous proof's new_sum.

#![no_main]
sp1_zkvm::entrypoint!(main);

use add_lib::PublicValuesStruct;
use alloy_sol_types::SolType;
use sha2::{Digest, Sha256};

pub fn main() {
    // Read the verification keys.
    let vkeys = sp1_zkvm::io::read::<Vec<[u32; 8]>>();

    // Read the public values.
    let public_values = sp1_zkvm::io::read::<Vec<Vec<u8>>>();

    // Verify that we have at least one proof
    assert!(
        !vkeys.is_empty(),
        "Must have at least one proof to aggregate"
    );
    assert_eq!(vkeys.len(), public_values.len());

    // Verify all the proofs using SP1's verify_sp1_proof
    // Note: The proofs must be read in the same order they were written
    for i in 0..vkeys.len() {
        let vkey = &vkeys[i];
        let public_values_bytes = &public_values[i];
        // Use SP1's patched SHA256 for optimal performance (automatically uses precompiles)
        let public_values_digest = Sha256::digest(public_values_bytes);
        // The proof data is automatically read when calling verify_sp1_proof
        sp1_zkvm::lib::verify::verify_sp1_proof(vkey, &public_values_digest.into());
    }

    // Parse all public values and verify the chain
    let mut decoded_values = Vec::new();
    for public_value_bytes in &public_values {
        let decoded = PublicValuesStruct::abi_decode(public_value_bytes.as_slice())
            .expect("Failed to decode public values");
        decoded_values.push(decoded);
    }

    // Verify that proofs form a valid chain: old_sum[i+1] == new_sum[i]
    for i in 0..decoded_values.len() - 1 {
        assert_eq!(
            decoded_values[i + 1].old_sum,
            decoded_values[i].new_sum,
            "Proof chain broken: proof {} new_sum ({}) != proof {} old_sum ({})",
            i,
            decoded_values[i].new_sum,
            i + 1,
            decoded_values[i + 1].old_sum
        );
    }

    // Create aggregated result: first old_sum and last new_sum
    let first_old_sum = decoded_values[0].old_sum;
    let last_new_sum = decoded_values[decoded_values.len() - 1].new_sum;

    let aggregated_values = PublicValuesStruct {
        old_sum: first_old_sum,
        new_sum: last_new_sum,
    };

    // Commit the aggregated public values
    let aggregated_bytes = PublicValuesStruct::abi_encode(&aggregated_values);
    sp1_zkvm::io::commit_slice(&aggregated_bytes);
}

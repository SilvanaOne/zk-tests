use crate::proof::{create_poseidon_circuit, PoseidonProof};
use kimchi::proof::ProverProof;
use kimchi::prover_index::testing::new_index_for_test;
use kimchi::verifier_index::VerifierIndex;
use mina_curves::pasta::Vesta;
use poly_commitment::ipa::OpeningProof as DlogOpeningProof;
use serde::Deserialize;
use std::error::Error;

type OpeningProof = DlogOpeningProof<Vesta>;

// Serialization methods are now defined in proof.rs impl block
pub fn deserialize_poseidon_proof(bytes: &[u8]) -> Result<PoseidonProof, Box<dyn Error>> {
    let mut cursor = std::io::Cursor::new(bytes);

    let mut deserializer = rmp_serde::Deserializer::new(&mut cursor);
    let proof = ProverProof::<Vesta, OpeningProof>::deserialize(&mut deserializer)?;

    let mut deserializer = rmp_serde::Deserializer::new(&mut cursor);
    let mut verifier_index = VerifierIndex::<Vesta, OpeningProof>::deserialize(&mut deserializer)?;

    // Recreate the SRS and other fields for the verifier index
    // The SRS and linearization data are not serialized, so we need to recreate them
    let gates = create_poseidon_circuit();
    let prover_index = new_index_for_test::<Vesta>(gates, 0);
    let fresh_verifier_index = prover_index.verifier_index();

    // Copy over the missing fields that aren't serialized properly
    verifier_index.srs = fresh_verifier_index.srs;
    verifier_index.linearization = fresh_verifier_index.linearization;
    verifier_index.powers_of_alpha = fresh_verifier_index.powers_of_alpha;

    Ok(PoseidonProof {
        proof,
        verifier_index,
    })
}

// Simple deserialization for zkVM that doesn't require SRS files
pub fn deserialize_poseidon_proof_zkvm(bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut cursor = std::io::Cursor::new(bytes);

    // Just deserialize the proof structure without recreating SRS
    let mut deserializer = rmp_serde::Deserializer::new(&mut cursor);
    let _proof = ProverProof::<Vesta, OpeningProof>::deserialize(&mut deserializer)?;

    // Deserialize verifier index
    let mut deserializer = rmp_serde::Deserializer::new(&mut cursor);
    let _verifier_index = VerifierIndex::<Vesta, OpeningProof>::deserialize(&mut deserializer)?;
    let _gates = create_poseidon_circuit();

    // We successfully deserialized the structures
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::{create_poseidon_proof, verify_poseidon_proof};

    #[test]
    fn test_serialize_deserialize_proof() -> Result<(), Box<dyn Error>> {
        println!("Creating original proof...");
        let original_proof = create_poseidon_proof()?;

        println!("Verifying original proof...");
        let original_valid = verify_poseidon_proof(&original_proof)?;
        assert!(original_valid, "Original proof should be valid");

        println!("Serializing proof to bytes...");
        let serialized_bytes = original_proof.serialize()?;
        println!("Serialized proof size: {} bytes", serialized_bytes.len());

        println!("Deserializing proof from bytes...");
        let deserialized_proof = deserialize_poseidon_proof(&serialized_bytes)?;

        println!("Verifying deserialized proof...");
        let deserialized_valid = verify_poseidon_proof(&deserialized_proof)?;
        assert!(deserialized_valid, "Deserialized proof should be valid");

        println!("✅ Serialization and deserialization successful!");
        println!("Both original and deserialized proofs are valid.");

        Ok(())
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() -> Result<(), Box<dyn Error>> {
        println!("Testing multiple serialization roundtrips...");

        let original_proof = create_poseidon_proof()?;
        let mut current_bytes = original_proof.serialize()?;

        for i in 1..=3 {
            println!("Roundtrip {}", i);
            let proof = deserialize_poseidon_proof(&current_bytes)?;
            let valid = verify_poseidon_proof(&proof)?;
            assert!(valid, "Proof should be valid after roundtrip {}", i);

            current_bytes = proof.serialize()?;
        }

        println!("✅ All roundtrips successful!");

        Ok(())
    }
}

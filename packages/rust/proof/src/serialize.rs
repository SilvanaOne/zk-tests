use crate::proof::{PoseidonProof, create_poseidon_proof, verify_poseidon_proof, create_poseidon_circuit};
use kimchi::proof::ProverProof;
use kimchi::verifier_index::VerifierIndex;
use kimchi::prover_index::testing::new_index_for_test;
use mina_curves::pasta::Vesta;
use poly_commitment::ipa::OpeningProof as DlogOpeningProof;
use serde::{Serialize, Deserialize};
use std::error::Error;

type OpeningProof = DlogOpeningProof<Vesta>;

impl PoseidonProof {
    pub fn serialize(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buf = Vec::new();
        
        let mut serializer = rmp_serde::Serializer::new(&mut buf);
        self.proof.serialize(&mut serializer)?;
        
        let mut serializer = rmp_serde::Serializer::new(&mut buf);
        self.verifier_index.serialize(&mut serializer)?;
        
        Ok(buf)
    }
    
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let deserialized_proof = PoseidonProof::deserialize(&serialized_bytes)?;
        
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
            let proof = PoseidonProof::deserialize(&current_bytes)?;
            let valid = verify_poseidon_proof(&proof)?;
            assert!(valid, "Proof should be valid after roundtrip {}", i);
            
            current_bytes = proof.serialize()?;
        }
        
        println!("✅ All roundtrips successful!");
        
        Ok(())
    }
}
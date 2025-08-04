use borsh::{BorshDeserialize, BorshSerialize};
use sp1_sdk::SP1ProofWithPublicValues;
use std::error::Error;

/// The instruction data for Solana program.
#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct SP1Groth16Proof {
    pub proof: Vec<u8>,
    pub sp1_public_inputs: Vec<u8>,
}

/// Convert SP1 proof to Solana-compatible format
pub fn convert_sp1_proof_for_solana(
    proof: SP1ProofWithPublicValues,
) -> Result<SP1Groth16Proof, Box<dyn Error>> {
    // Ensure the proof is a Groth16 proof
    if !matches!(proof.proof, sp1_sdk::SP1Proof::Groth16(_)) {
        return Err("Proof must be a Groth16 proof for Solana verification".into());
    }

    // Create the Solana-compatible proof structure
    let solana_proof = SP1Groth16Proof {
        proof: proof.bytes(),
        sp1_public_inputs: proof.public_values.to_vec(),
    };

    Ok(solana_proof)
}

/// Create fixture data for Solana verification
pub fn create_solana_fixture(
    proof: &SP1ProofWithPublicValues,
    vkey_hash: &str,
) -> Result<SolanaProofFixture, Box<dyn Error>> {
    let solana_proof = convert_sp1_proof_for_solana(proof.clone())?;
    
    Ok(SolanaProofFixture {
        vkey_hash: vkey_hash.to_string(),
        proof_bytes: solana_proof.proof,
        public_inputs_bytes: solana_proof.sp1_public_inputs,
    })
}

/// Solana proof fixture for testing
#[derive(Debug, Clone)]
pub struct SolanaProofFixture {
    pub vkey_hash: String,
    pub proof_bytes: Vec<u8>,
    pub public_inputs_bytes: Vec<u8>,
}
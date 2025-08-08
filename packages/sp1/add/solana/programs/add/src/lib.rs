use anchor_lang::prelude::*;
use sp1_solana::verify_proof;

declare_id!("DrENg7J4SEZbTi419ZA1AnXFzh8wehfwisapdCeTEpqt");

/// The instruction data for the program.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SP1Groth16Proof {
    pub proof: Vec<u8>,
    pub sp1_public_inputs: Vec<u8>,
}

/// Verification key hash for the add program
/// This should be updated with the actual vkey hash from your SP1 program
const ADD_VKEY_HASH: &str = "0x00bee99e7cb561bd60cb0bb43002e9ae74ff8769c756fd82e6a4b18d990f7680";
#[program]
pub mod add {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Add Solana program: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn verify_add_proof(_ctx: Context<VerifyProof>, proof_data: SP1Groth16Proof) -> Result<()> {
        // Get the SP1 Groth16 verification key from the `sp1-solana` crate
        let vk = sp1_solana::GROTH16_VK_5_0_0_BYTES;

        // Verify the proof
        verify_proof(
            &proof_data.proof,
            &proof_data.sp1_public_inputs,
            &ADD_VKEY_HASH,
            vk,
        )
        .map_err(|_| error!(ErrorCode::InvalidProof))?;

        // Deserialize and log the public values
        let reader = proof_data.sp1_public_inputs.as_slice();

        // SP1 public values are serialized as 32-byte values representing uint256
        // Read old_root (first 32 bytes)
        let old_root_bytes = &reader[0..32];
        // Read new_root (second 32 bytes)
        let new_root_bytes = &reader[32..64];

        msg!("SP1 proof verified successfully!");
        msg!("Public values: old_root: 0x{}", hex::encode(old_root_bytes));
        msg!("Public values: new_root: 0x{}", hex::encode(new_root_bytes));

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct VerifyProof {}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid SP1 proof")]
    InvalidProof,
}

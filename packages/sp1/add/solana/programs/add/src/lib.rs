use anchor_lang::prelude::*;
use sp1_solana::verify_proof;

declare_id!("BTbMTALLVaSor7BfPTgDoFJvqmMAePHgs6HdZRdv4B1x");

/// The instruction data for the program.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SP1Groth16Proof {
    pub proof: Vec<u8>,
    pub sp1_public_inputs: Vec<u8>,
}

/// Verification key hash for the add program
/// This should be updated with the actual vkey hash from your SP1 program
const ADD_VKEY_HASH: &str = "0x004bb10cab9d6cbd507923397b000ca182e137d088000b635c8e2cae2e80fbc4";

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
        
        // SP1 public values are serialized as 32-byte values
        // Read old_sum (32 bytes, but we only need the last 4 bytes for u32)
        let old_sum_bytes = &reader[0..32];
        let old_sum = u32::from_be_bytes([
            old_sum_bytes[28],
            old_sum_bytes[29],
            old_sum_bytes[30],
            old_sum_bytes[31],
        ]);
        
        // Read new_sum (32 bytes, but we only need the last 4 bytes for u32)
        let new_sum_bytes = &reader[32..64];
        let new_sum = u32::from_be_bytes([
            new_sum_bytes[28],
            new_sum_bytes[29],
            new_sum_bytes[30],
            new_sum_bytes[31],
        ]);
        
        msg!("SP1 proof verified successfully!");
        msg!("Public values: old_sum: {}, new_sum: {}", old_sum, new_sum);

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

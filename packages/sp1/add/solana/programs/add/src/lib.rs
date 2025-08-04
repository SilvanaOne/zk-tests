use anchor_lang::prelude::*;

declare_id!("BTbMTALLVaSor7BfPTgDoFJvqmMAePHgs6HdZRdv4B1x");

#[program]
pub mod add {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Add Solana program: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

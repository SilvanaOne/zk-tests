use anchor_lang::prelude::*;
use hash::PoseidonHash;
use num_bigint::BigInt;

mod hash;

declare_id!("FGk1Bifkhbu6kY99SRLZpNzsXnbgQjXGCmhfYP4zCunT");

#[program]

pub mod poseidon {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn poseidon(_ctx: Context<Poseidon>, a: u8, b: u8, count: u32) -> Result<()> {
        // let mut i = 0;
        // while i < count {
            let input = vec![BigInt::from(1), BigInt::from(2)];
            let hash_result = PoseidonHash::hash(input);
            msg!("Poseidon hash of [1,2]: {}", hash_result);
        //     i += 1;
        // }

        msg!(
            "Calculated Mina Poseidon hash of {} and {} {} times",
            a,
            b,
            count
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct Poseidon {}

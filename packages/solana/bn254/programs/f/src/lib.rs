use anchor_lang::prelude::*;
use solana_poseidon::{hash, Endianness, Parameters};

declare_id!("9cK1EqXi1KRhX4WVqwoqzoBrp6CCQjrFJSxmMDEVauui");

#[program]
pub mod f {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn poseidon(_ctx: Context<Poseidon>, a: u8, b: u8, count: u32) -> Result<()> {
        let mut i = 0;
        while i < count {
            hash(Parameters::Bn254X5, Endianness::BigEndian, &[a, b]).unwrap();
            i += 1;
        }

        msg!(
            "Calculated Poseidon BN254 hash of {} and {} {} times",
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

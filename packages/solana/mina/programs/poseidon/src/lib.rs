use anchor_lang::prelude::*;
use mina_hasher::{create_kimchi, Fp, Hashable, Hasher, ROInput};

declare_id!("EdsRD7dwER75GnzH9gw6kJjwr1pxTGtQLeVBSxLLiNWY");

#[derive(Debug, Clone)]
struct PoseidonInput {
    a: Fp,
    b: Fp,
}

impl Hashable for PoseidonInput {
    type D = ();

    fn to_roinput(&self) -> ROInput {
        ROInput::new().append_field(self.a).append_field(self.b)
    }

    fn domain_string(_: Self::D) -> Option<String> {
        // format!("PoseidonInput").into()
        None
    }
}

#[program]

pub mod poseidon {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn poseidon(_ctx: Context<Poseidon>, a: u8, b: u8, count: u32) -> Result<()> {
        let poseidon_input = PoseidonInput {
            a: Fp::from(a),
            b: Fp::from(b),
        };
        let mut hasher = create_kimchi::<PoseidonInput>(());
        //let hash = hasher.hash(&poseidon_input);
        let mut i = 0;
        while i < count {
            let hash = hasher.hash(&poseidon_input);
            msg!("Hash: {:?}", hash.to_string()); // Hash: "17017029585017630513954937283105772963331887127320430819007921583560430366787"
            i += 1;
        }

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

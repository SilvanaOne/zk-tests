use crate::hash::PoseidonInput;
use mina_signer::{self, Keypair, PubKey, Signature, Signer};

pub fn sign_fields(data: &PoseidonInput, keypair: &Keypair) -> Signature {
    let mut ctx = mina_signer::create_kimchi::<PoseidonInput>(());
    ctx.sign(&keypair, &data)
}

pub fn verify_fields(data: &PoseidonInput, public_key: &PubKey, signature: &Signature) -> bool {
    let mut ctx = mina_signer::create_legacy::<PoseidonInput>(());
    ctx.verify(&signature, &public_key, &data)
}

use mina_signer::Keypair;
use rand_core::OsRng;

pub fn generate_keypair() -> Keypair {
    let mut rng = OsRng;
    Keypair::rand(&mut rng).unwrap()
}

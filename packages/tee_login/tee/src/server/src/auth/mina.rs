use mina_signer::Keypair;
use rand::rngs::OsRng;

pub type KeyPair = Keypair;

pub fn generate_keypair() -> Keypair {
    let mut rng = OsRng {};
    Keypair::rand(&mut rng).unwrap()
}

pub fn to_public_key_base58_string(key: &Keypair) -> String {
    key.clone().get_address()
}

pub fn to_private_key_base58_string(key: &Keypair) -> String {
    key.clone().secret.to_base58()
}

use crate::auth::{ethereum, mina, solana, sui};

#[derive(Debug)]
pub struct Keys {
    pub solana_keypair: solana::KeyPair,
    pub sui_keypair: sui::KeyPair,
    pub mina_keypair: mina::KeyPair,
    pub ethereum_keypair: ethereum::KeyPair,
}
#[derive(Debug, Clone)]
pub struct Addresses {
    pub solana_address: String,
    pub sui_address: String,
    pub mina_address: String,
    pub ethereum_address: String,
}

impl Keys {
    pub fn new() -> Keys {
        let solana_keypair = solana::create_keypair();
        let sui_keypair = sui::create_keypair();
        let mina_keypair = mina::generate_keypair();
        let ethereum_keypair = ethereum::create_keypair();
        Keys {
            solana_keypair,
            sui_keypair,
            mina_keypair,
            ethereum_keypair,
        }
    }
    pub fn to_addresses(&self) -> Addresses {
        Addresses {
            solana_address: solana::to_public_key_base58_string(&self.solana_keypair),
            sui_address: sui::to_address(&self.sui_keypair),
            mina_address: mina::to_public_key_base58_string(&self.mina_keypair),
            ethereum_address: self.ethereum_keypair.address.clone(),
        }
    }
}

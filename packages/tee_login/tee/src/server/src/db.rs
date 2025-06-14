use serde::{Deserialize, Serialize};

/// Composite key: (chain, wallet, address)
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Key {
    pub chain: String,
    pub wallet: String,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Share {
    pub index: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Value {
    pub created_at: u64,
    pub expiry: u64,
    pub shares: Vec<Share>,
}



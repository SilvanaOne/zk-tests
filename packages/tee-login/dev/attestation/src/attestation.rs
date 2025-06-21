use crate::nitro_attestation::{parse_nitro_attestation, verify_nitro_attestation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Addresses {
    pub solana_address: String,
    pub sui_address: String,
    pub mina_address: String,
    pub ethereum_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attestation {
    pub is_valid: bool,
    pub digest: String,
    pub timestamp: u64,
    pub module_id: String,
    pub public_key: Option<Vec<u8>>,
    pub user_data: Option<Vec<u8>>,
    pub nonce: Option<Vec<u8>>,
    pub pcr_vec: Vec<String>,
    pub pcr_map: HashMap<u8, String>,
    pub addresses: Option<Addresses>,
}

pub fn verify_attestation(attestation: &str) -> Result<Attestation, Box<dyn std::error::Error>> {
    // Handle hex decoding with proper error handling
    let attestation_bytes = match hex::decode(attestation) {
        Ok(bytes) => bytes,
        Err(e) => return Err(format!("Failed to decode hex attestation: {}", e).into()),
    };

    // Handle attestation parsing with proper error handling
    let (vec0, vec1, attestation) = match parse_nitro_attestation(&attestation_bytes, true) {
        Ok(result) => result,
        Err(e) => return Err(format!("Failed to parse nitro attestation: {}", e).into()),
    };

    // Handle address deserialization with proper error handling
    let addresses: Option<Addresses> = if let Some(user_data_bytes) = &attestation.user_data {
        match bincode::deserialize::<Addresses>(user_data_bytes) {
            Ok(addr) => Some(addr),
            Err(e) => {
                // Log the error but don't fail the entire operation
                eprintln!(
                    "Warning: Failed to deserialize addresses from user_data: {}",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let pcr_vec = attestation
        .pcr_vec
        .iter()
        .map(|v| hex::encode(v))
        .collect::<Vec<String>>();
    let pcr_map = attestation
        .pcr_map
        .iter()
        .map(|(k, v)| (*k, hex::encode(v)))
        .collect::<HashMap<u8, String>>();

    let res = verify_nitro_attestation(
        &vec0,
        &vec1,
        &attestation,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    );

    Ok(Attestation {
        is_valid: res.is_ok(),
        digest: attestation.digest,
        timestamp: attestation.timestamp,
        module_id: attestation.module_id,
        public_key: attestation.public_key,
        user_data: attestation.user_data,
        nonce: attestation.nonce,
        pcr_vec,
        pcr_map,
        addresses: addresses,
    })
}

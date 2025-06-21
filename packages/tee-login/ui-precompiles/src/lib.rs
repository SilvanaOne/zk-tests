use crate::attestation::Attestation;
use bc_shamir::recover_secret;
use bip39::Mnemonic;
use js_sys::{Array, Uint8Array};
use mina_hasher::Fp;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use wasm_bindgen::prelude::*;
use web_sys::console;
use zeroize::Zeroizing;

use crate::hash::PoseidonInput;
use crate::keypair::generate_keypair;
use crate::signer::sign_fields;

mod attestation;
mod hash;
mod keypair;
mod nitro_attestation;
mod signer;

// Convenient macro for console logging
macro_rules! console_log {
    ($($t:tt)*) => (console::log_1(&format!($($t)*).into()))
}

#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Share {
    pub index: u32,
    pub data: Vec<u8>,
}

#[wasm_bindgen]
pub fn recover_mnemonic(shares_serialized: &Array) -> Option<String> {
    let mut shares: Vec<Share> = Vec::new();

    for (i, item) in shares_serialized.iter().enumerate() {
        match item.dyn_ref::<Uint8Array>() {
            Some(uint8_array) => {
                let bytes = uint8_array.to_vec();

                match bincode::deserialize::<Share>(&bytes) {
                    Ok(share) => {
                        shares.push(share);
                    }
                    Err(e) => {
                        console_log!("rust: failed to deserialize share {}: {:?}", i, e);
                        return None;
                    }
                }
            }
            None => {
                console_log!("rust: item {} is not a Uint8Array", i);
                return None;
            }
        }
    }

    console_log!("rust: successfully processed {} shares", shares.len());

    // Use sequential indices for the shares we have
    let indexes: Vec<u32> = shares.iter().map(|s| s.index).collect();
    let secret = Zeroizing::new(
        recover_secret(
            &indexes.iter().map(|i| *i as usize).collect::<Vec<_>>(),
            &shares.iter().map(|s| s.data.clone()).collect::<Vec<_>>(),
        )
        .ok()?,
    );
    // secret is Vec<u8>; convert back to mnemonic
    let mnemonic = Zeroizing::new(Mnemonic::from_entropy(&secret).ok()?);
    Some(mnemonic.to_string())
}

#[wasm_bindgen]
pub fn signature() -> String {
    let keypair = generate_keypair();

    // let secret_key =
    //     SecKey::from_base58("EKEtdWo2dFqNx6qEhbeVaHkYbcnLhz3pXkS4mjQyfJdPFZB3onG5").unwrap();
    // let keypair = Keypair::from_secret_key(secret_key).unwrap();
    let address = &keypair.clone().get_address();
    println!("Address: {:?}", address);
    let private_key = &keypair.clone().secret;
    let private_key_str = private_key.to_base58();
    println!("Private key: {:?}", private_key_str);
    let public_key = &keypair.clone().public;
    let public_key_str = public_key.to_string();
    println!("Public key: {:?}", public_key_str);
    let msg = PoseidonInput {
        data: [
            Fp::from_str("240717916736854602989207148466022993262069182275").unwrap(),
            Fp::from(1),
            Fp::from(2), //Fp::from(3),
        ],
    };
    let signature = sign_fields(&msg, &keypair);
    println!("Signature: {:?}", signature);
    signature.to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttestationVerificationResult {
    pub result: Option<Attestation>,
    pub error: Option<String>,
}

#[wasm_bindgen]
pub fn verify_attestation(attestation: &str) -> String {
    let result = match attestation::verify_attestation(attestation) {
        Ok(att) => AttestationVerificationResult {
            result: Some(att),
            error: None,
        },
        Err(e) => AttestationVerificationResult {
            result: None,
            error: Some(e.to_string()),
        },
    };
    serde_json::to_string(&result)
        .unwrap_or_else(|e| format!(r#"{{"error": "Serialization failed: {}"}}"#, e))
}

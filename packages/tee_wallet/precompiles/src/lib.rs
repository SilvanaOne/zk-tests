use bc_shamir::recover_secret;
use bip39::Mnemonic;
use js_sys::{Array, Uint8Array};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::console;
use zeroize::Zeroizing;

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

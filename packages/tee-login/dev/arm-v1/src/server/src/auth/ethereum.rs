use crate::login::VerifyResult;
use ethers_core::types::H160;
use ethers_core::utils::keccak256;
use hex;
use rand::Rng;
use secp256k1::{
    Message, Secp256k1, SecretKey,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use std::convert::TryFrom;

#[derive(Debug, Clone)]
pub struct KeyPair {
    pub address: String,
    pub private_key: String,
}

pub fn create_keypair() -> KeyPair {
    let secp = Secp256k1::new();
    let mut rng = rand::thread_rng();

    // Generate a random 32-byte private key
    let mut private_key_bytes = [0u8; 32];
    rng.fill(&mut private_key_bytes);

    let secret_key =
        SecretKey::from_slice(&private_key_bytes).expect("32 bytes, within curve order");
    let public_key = secret_key.public_key(&secp);

    // Get the uncompressed public key (65 bytes starting with 0x04)
    let public_key_uncompressed = public_key.serialize_uncompressed();

    // Hash the public key (without the 0x04 prefix) and take last 20 bytes for address
    let hashed_pubkey = keccak256(&public_key_uncompressed[1..]);
    let address = H160::from_slice(&hashed_pubkey[12..]);

    // Return address with 0x prefix and private key as hex
    let address_string = format!("0x{}", hex::encode(address));
    let private_key_string = hex::encode(private_key_bytes);

    KeyPair {
        address: address_string,
        private_key: private_key_string,
    }
}

pub fn to_address_string(private_key_hex: &str) -> Result<String, Box<dyn std::error::Error>> {
    let secp = Secp256k1::new();
    let private_key_bytes = hex::decode(private_key_hex)?;
    let secret_key = SecretKey::from_slice(&private_key_bytes)?;
    let public_key = secret_key.public_key(&secp);

    let public_key_uncompressed = public_key.serialize_uncompressed();
    let hashed_pubkey = keccak256(&public_key_uncompressed[1..]);
    let address = H160::from_slice(&hashed_pubkey[12..]);

    Ok(format!("0x{}", hex::encode(address)))
}

pub fn to_private_key_string(private_key_hex: &str) -> String {
    // Simply return the hex string (this function keeps the same interface)
    private_key_hex.to_string()
}

pub fn verify_signature(
    address: &str,
    signature: &str,
    message: &str,
) -> Result<VerifyResult, Box<dyn std::error::Error>> {
    {
        // Decode the signature (strip 0x if present)
        let sig = signature.strip_prefix("0x").unwrap_or(signature);
        let sig_bytes = hex::decode(sig)?;

        // Ethereum `personal_sign` signatures are 65 bytes (r || s || v)
        if sig_bytes.len() != 65 {
            return Ok(VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some("Invalid signature length".into()),
            });
        }

        // Split signature into r||s (first 64 bytes) and v (last byte)
        let mut sig_rs = [0u8; 64];
        sig_rs.copy_from_slice(&sig_bytes[..64]);
        let v = sig_bytes[64];
        // Normalise to raw recovery‑id in range 0..=3
        let rec_id_byte = match v {
            27 | 28 => v - 27, // legacy Metamask quirk
            0 | 1 => v,
            _ => {
                return Ok(VerifyResult {
                    is_valid: false,
                    address: None,
                    nonce: None,
                    error: Some("Invalid recovery id".into()),
                });
            }
        };

        // Build recoverable signature object
        let rec_id = RecoveryId::try_from(rec_id_byte as i32)?;
        let rec_sig = RecoverableSignature::from_compact(&sig_rs, rec_id)?;

        // "\x19Ethereum Signed Message:\n" + len(message) || message
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        let prefixed_msg = [prefix.as_bytes(), message.as_bytes()].concat();
        let digest = keccak256(&prefixed_msg); // returns [u8; 32]
        let secp = Secp256k1::new();
        let msg = Message::from_digest(digest);
        let pubkey = secp.recover_ecdsa(&msg, &rec_sig)?;

        // Derive address from the public key (Keccak256 of the uncompressed key, last 20 bytes)
        let pubkey_uncompressed = pubkey.serialize_uncompressed();
        let hashed_pubkey = keccak256(&pubkey_uncompressed[1..]); // skip the 0x04 prefix
        let recovered_address = H160::from_slice(&hashed_pubkey[12..]);

        // Compare to supplied address (case‑insensitive)
        let supplied = address.trim_start_matches("0x").to_lowercase();
        let recovered = hex::encode(recovered_address);

        Ok(VerifyResult {
            is_valid: supplied == recovered,
            address: Some(recovered),
            nonce: None,
            error: None,
        })
    }
}

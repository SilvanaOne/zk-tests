use crate::login::VerifyResult;
use bs58;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
// use solana_sdk::pubkey::Pubkey;
// use solana_sdk::signature::Signature;
// use std::str::FromStr;

pub type KeyPair = SigningKey;

pub fn create_keypair() -> SigningKey {
    let mut csprng = rand::thread_rng();
    SigningKey::generate(&mut csprng)
}

pub fn to_public_key_base58_string(key: &SigningKey) -> String {
    let public_key = key.verifying_key();
    bs58::encode(public_key.to_bytes()).into_string()
}

pub fn to_private_key_base58_string(key: &SigningKey) -> String {
    bs58::encode(key.to_bytes()).into_string()
}

pub fn verify_signature(
    address: &str,
    signature: &str,
    message: &str,
) -> Result<VerifyResult, Box<dyn std::error::Error>> {
    // let pubkey = Pubkey::from_str(address)?;
    let signature_bytes = hex::decode(signature)?;
    if signature_bytes.len() != 64 {
        return Ok(VerifyResult {
            is_valid: false,
            address: None,
            nonce: None,
            error: Some("Invalid signature length".into()),
        });
    }

    let message_bytes = message.as_bytes().to_vec();
    // let signature = Signature::try_from(signature_bytes.as_slice())?;
    // let is_valid = signature.verify(pubkey.as_ref(), &message_bytes);

    // Decode the base58 encoded address to bytes
    let pubkey_bytes = bs58::decode(address).into_vec()?;
    let publickey = VerifyingKey::from_bytes(
        &pubkey_bytes
            .try_into()
            .map_err(|_| "Invalid public key length")?,
    )?;
    let dalek_signature = Signature::from_slice(&signature_bytes)?;
    let is_valid = publickey
        .verify_strict(&message_bytes, &dalek_signature)
        .is_ok();

    Ok(VerifyResult {
        is_valid,
        address: Some(address.to_string()),
        nonce: None,
        error: None,
    })
}

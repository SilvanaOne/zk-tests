use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use sui_sdk_types as sui;

pub struct GeneratedKeypair {
    pub secret_key: [u8; 32],
    pub public_key: [u8; 32],
    pub address: sui::Address,
    pub suiprivkey_bech32: String,
}

pub fn generate_ed25519() -> GeneratedKeypair {
    let mut csprng = OsRng;
    let mut sk_bytes = [0u8; 32];
    csprng.fill_bytes(&mut sk_bytes);

    let signing_key = SigningKey::from_bytes(&sk_bytes);
    let verifying_key = signing_key.verifying_key();
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(verifying_key.as_bytes());

    // Compute Sui address from [flag || pubkey]
    let mut hasher = Blake2b512::new();
    hasher.update([0x00u8]);
    hasher.update(&pk_bytes);
    let full = hasher.finalize();
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&full[..32]);
    let address = sui::Address::from_bytes(&addr_bytes).expect("address length");

    // suiprivkey bech32 encoding: [flag || 32-byte secret]
    let mut payload = Vec::with_capacity(33);
    payload.push(0x00);
    payload.extend_from_slice(&sk_bytes);
    let suiprivkey_bech32 = bech32::encode(
        "suiprivkey",
        bech32::ToBase32::to_base32(&payload),
        bech32::Variant::Bech32,
    )
    .expect("bech32 encode");

    GeneratedKeypair {
        secret_key: sk_bytes,
        public_key: pk_bytes,
        address,
        suiprivkey_bech32,
    }
}

pub fn sign_message(secret_key: &[u8; 32], message: &[u8]) -> Vec<u8> {
    let signing_key = SigningKey::from_bytes(secret_key);
    let verifying_key = signing_key.verifying_key();
    let sig: Signature = signing_key.sign(message);

    let mut sui_sig = Vec::with_capacity(97);
    sui_sig.push(0x00); // ed25519 flag
    sui_sig.extend_from_slice(&sig.to_bytes());
    sui_sig.extend_from_slice(verifying_key.as_bytes());
    sui_sig
}

pub fn verify_with_address(address: &sui::Address, message: &[u8], sui_sig: &[u8]) -> bool {
    if sui_sig.len() != 97 {
        return false;
    }
    if sui_sig[0] != 0x00 {
        return false;
    }
    let sig_bytes: [u8; 64] = match sui_sig[1..65].try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let pk_bytes: [u8; 32] = match sui_sig[65..97].try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Recompute address
    let mut hasher = Blake2b512::new();
    hasher.update([0x00u8]);
    hasher.update(&pk_bytes);
    let full = hasher.finalize();
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&full[..32]);
    let derived = match sui::Address::from_bytes(&addr_bytes) {
        Ok(a) => a,
        Err(_) => return false,
    };
    if &derived != address {
        return false;
    }

    // Verify signature
    let vk = match VerifyingKey::from_bytes(&pk_bytes) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let sig = Signature::from_bytes(&sig_bytes);
    vk.verify(message, &sig).is_ok()
}

pub fn bcs_serialize<T: Serialize>(payload: &T) -> Result<Vec<u8>, bcs::Error> {
    bcs::to_bytes(payload)
}

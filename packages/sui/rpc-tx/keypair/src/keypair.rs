use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use sui_sdk_types as sui;

pub struct GeneratedKeypair {
    pub secret_key: [u8; 32],
    pub public_key: [u8; 32],
    pub address: sui::Address,
    pub sui_private_key: String,
}

pub fn generate_ed25519() -> GeneratedKeypair {
    let mut csprng = OsRng;
    let mut sk_bytes = [0u8; 32];
    csprng.fill_bytes(&mut sk_bytes);

    let signing_key = SigningKey::from_bytes(&sk_bytes);
    let verifying_key = signing_key.verifying_key();
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(verifying_key.as_bytes());

    // Compute Sui address using SDK
    let sui_public_key = sui::Ed25519PublicKey::new(pk_bytes);
    let address = sui_public_key.derive_address();

    // suiprivkey bech32 encoding: [flag || 32-byte secret]
    let mut payload = Vec::with_capacity(33);
    payload.push(0x00);
    payload.extend_from_slice(&sk_bytes);
    let sui_private_key = bech32::encode(
        "suiprivkey",
        bech32::ToBase32::to_base32(&payload),
        bech32::Variant::Bech32,
    )
    .expect("bech32 encode");

    GeneratedKeypair {
        secret_key: sk_bytes,
        public_key: pk_bytes,
        address,
        sui_private_key,
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

    // Recompute address using SDK
    let sui_public_key = sui::Ed25519PublicKey::new(pk_bytes);
    let derived = sui_public_key.derive_address();
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

pub fn from_sui_private_key(sui_private_key: &str) -> Result<GeneratedKeypair, String> {
    // Decode bech32
    let (hrp, data, _) =
        bech32::decode(sui_private_key).map_err(|e| format!("Failed to decode bech32: {}", e))?;

    if hrp != "suiprivkey" {
        return Err("Invalid HRP, expected 'suiprivkey'".to_string());
    }

    let decoded: Vec<u8> = bech32::FromBase32::from_base32(&data)
        .map_err(|e| format!("Failed to decode base32: {}", e))?;

    if decoded.len() != 33 {
        return Err("Invalid private key length".to_string());
    }

    if decoded[0] != 0x00 {
        return Err("Invalid flag byte".to_string());
    }

    let mut sk_bytes = [0u8; 32];
    sk_bytes.copy_from_slice(&decoded[1..33]);

    // Derive public key and address
    let signing_key = SigningKey::from_bytes(&sk_bytes);
    let verifying_key = signing_key.verifying_key();
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(verifying_key.as_bytes());

    // Compute Sui address using SDK
    let sui_public_key = sui::Ed25519PublicKey::new(pk_bytes);
    let address = sui_public_key.derive_address();

    Ok(GeneratedKeypair {
        secret_key: sk_bytes,
        public_key: pk_bytes,
        address,
        sui_private_key: sui_private_key.to_string(),
    })
}

pub fn bcs_serialize<T: Serialize>(payload: &T) -> Result<Vec<u8>, bcs::Error> {
    bcs::to_bytes(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_sui_private_key_produces_expected_address() {
        let secret_key = "suiprivkey1qqg9ex8p8e8fdz2ex5r0muptts3e4zctv8eahdxcrl5vne73szs365yfhkp";
        let expected_address = "0xdaf4e0011c0df11dfca353dd9e11124f0a9a08e622787c3210f773b0d5312174";

        let keypair = from_sui_private_key(secret_key).expect("Failed to parse private key");
        let address_hex = format!("0x{}", hex::encode(keypair.address.as_bytes()));

        assert_eq!(address_hex, expected_address);
    }

    #[test]
    fn test_from_sui_private_key_produces_expected_address_2() {
        let secret_key = "suiprivkey1qzxr2y0cwppjeqkrjfjy7nyskxf2k23zsjx3pawde6f45egw5szns52cxra";
        let expected_address = "0xbd22aa69c59813435088fa59b5fc5018a434fa9714dcf46108271682d89f7393";

        let keypair = from_sui_private_key(secret_key).expect("Failed to parse private key");
        let address_hex = format!("0x{}", hex::encode(keypair.address.as_bytes()));

        assert_eq!(address_hex, expected_address);
    }
}

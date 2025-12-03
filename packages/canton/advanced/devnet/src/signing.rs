//! Ed25519 signing utilities for external party authentication.
//!
//! Supports Base58-encoded private keys (64 bytes: seed + public key).

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signer, SigningKey};

/// Parse Base58-encoded Ed25519 private key and return 32-byte seed.
///
/// The Base58 key decodes to 64 bytes: first 32 bytes are the seed,
/// last 32 bytes are the public key.
pub fn parse_base58_private_key(base58_key: &str) -> Result<[u8; 32]> {
    let decoded = bs58::decode(base58_key)
        .into_vec()
        .map_err(|e| anyhow!("Failed to decode Base58 private key: {}", e))?;

    if decoded.len() < 32 {
        return Err(anyhow!(
            "Private key too short: expected at least 32 bytes, got {}",
            decoded.len()
        ));
    }

    let mut seed = [0u8; 32];
    seed.copy_from_slice(&decoded[..32]);
    Ok(seed)
}

/// Sign a transaction hash with Ed25519.
///
/// Input: hash as Base64 string (from preparedTransactionHash)
/// Output: signature as Base64 string (64 bytes)
pub fn sign_transaction_hash(seed: &[u8; 32], hash_base64: &str) -> Result<String> {
    let signing_key = SigningKey::from_bytes(seed);
    let hash_bytes = BASE64
        .decode(hash_base64)
        .map_err(|e| anyhow!("Failed to decode Base64 hash: {}", e))?;

    let signature = signing_key.sign(&hash_bytes);
    Ok(BASE64.encode(signature.to_bytes()))
}

/// Extract fingerprint from party ID (part after ::).
///
/// Party ID format: "namespace::fingerprint"
/// Example: "ext-user-phantom-1::12208bf4c7bd..." -> "12208bf4c7bd..."
pub fn get_fingerprint(party_id: &str) -> Result<String> {
    party_id
        .split("::")
        .nth(1)
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Invalid party ID format: expected 'namespace::fingerprint'"))
}

/// Extract userId (sub claim) from JWT token.
///
/// The userId in Canton interactive submission requests must match
/// the sub claim in the JWT used for authentication.
pub fn extract_user_id_from_jwt(jwt: &str) -> Result<String> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid JWT format: expected 3 parts"));
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| anyhow!("Failed to decode JWT payload: {}", e))?;

    let claims: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| anyhow!("Failed to parse JWT claims: {}", e))?;

    claims
        .get("sub")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing 'sub' claim in JWT"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_fingerprint() {
        let party_id = "ext-user-phantom-1::12208bf4c7bd06398a912a294ecf703f22b6ba1f10d83088134b49b1539505aa21df";
        let fingerprint = get_fingerprint(party_id).unwrap();
        assert_eq!(
            fingerprint,
            "12208bf4c7bd06398a912a294ecf703f22b6ba1f10d83088134b49b1539505aa21df"
        );
    }

    #[test]
    fn test_parse_base58_key() {
        // Test with a known Base58 key
        let key = "EB92Q6V2a78t9ppqMuKLppyfzFgyYJciQEVHZKnXAhjEwVpx9aMbQN84SR4ceo3mbLUxQF7TLzaEujaTJnS7eRF";
        let seed = parse_base58_private_key(key).unwrap();
        assert_eq!(seed.len(), 32);
    }
}

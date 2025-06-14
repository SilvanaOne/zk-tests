use crate::login::LoginRequest;
use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use sha2::{Digest, Sha256};

pub fn hash_login_request(login_request: &LoginRequest) -> bool {
    // Extract hash from message: "Silvana TEE login request: {hash}"
    let prefix = "Silvana TEE login request: ";
    let extracted_hash = match login_request.message.strip_prefix(prefix) {
        Some(hash) => hash,
        None => return false, // Invalid message format
    };

    // Create metadata JSON in the same format as TypeScript
    let metadata = json!({
        "domain": "https://login.silvana.dev",
        "login_type": login_request.login_type,
        "chain": login_request.chain,
        "wallet": login_request.wallet,
        "address": login_request.address,
        "publicKey": login_request.public_key,
        "nonce": login_request.nonce,
    });

    // Convert to JSON string (compact format, no spaces)
    let metadata_str = metadata.to_string();

    // Hash the metadata with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(metadata_str.as_bytes());
    let hash_bytes = hasher.finalize();

    // Encode to base64
    let calculated_hash = general_purpose::STANDARD.encode(&hash_bytes);

    // Compare the extracted hash with the calculated hash
    extracted_hash == calculated_hash
}

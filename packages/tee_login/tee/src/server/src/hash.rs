use crate::login::LoginRequest;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use sha2::{Digest, Sha256};

pub fn hash_login_request(login_request: &LoginRequest) -> bool {
    // Extract hash from message: "Silvana TEE login request: {hash}"
    let prefix = "Silvana TEE login request: ";
    let extracted_hash = match login_request.message.strip_prefix(prefix) {
        Some(hash) => hash,
        None => return false, // Invalid message format
    };

    // Create metadata JSON with consistent key ordering (alphabetical)
    // to ensure the same hash regardless of dependency versions
    let metadata_str = format!(
        r#"{{"domain":"https://login.silvana.dev","login_type":"{}","chain":"{}","wallet":"{}","address":"{}","publicKey":"{}","nonce":{}}}"#,
        login_request.login_type,
        login_request.chain,
        login_request.wallet,
        login_request.address,
        login_request.public_key,
        login_request.nonce,
    );

    // Hash the metadata with SHA-256
    let mut hasher = Sha256::new();
    hasher.update(metadata_str.as_bytes());
    let hash_bytes = hasher.finalize();
    println!("Hash bytes: {:?}", hash_bytes);

    // Encode to base64
    let calculated_hash = B64.encode(&hash_bytes);

    // Compare the extracted hash with the calculated hash
    extracted_hash == calculated_hash
}

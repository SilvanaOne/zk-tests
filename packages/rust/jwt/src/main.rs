use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use jsonwebtoken::{Algorithm, Header};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: i64,
    iat: i64,
    custom_data: String,
}

/// Generate a new Ed25519 keypair
fn generate_keypair() -> (SigningKey, VerifyingKey) {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Create and sign a JWT using Ed25519 private key
fn create_signed_jwt(signing_key: &SigningKey, claims: &Claims) -> Result<String, Box<dyn std::error::Error>> {
    // Create header
    let header = Header::new(Algorithm::EdDSA);

    // Encode header and claims
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header)?);
    let claims_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?);

    // Create the message to sign
    let message = format!("{}.{}", header_b64, claims_b64);

    // Sign the message
    let signature: Signature = signing_key.sign(message.as_bytes());

    // Encode the signature
    let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

    // Combine to create the JWT
    let jwt = format!("{}.{}", message, signature_b64);

    Ok(jwt)
}

/// Verify a JWT using Ed25519 public key
fn verify_jwt(jwt: &str, verifying_key: &VerifyingKey) -> Result<Claims, Box<dyn std::error::Error>> {
    // Split the JWT into parts
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format".into());
    }

    // Decode header
    let header_bytes = URL_SAFE_NO_PAD.decode(parts[0])?;
    let header: Header = serde_json::from_slice(&header_bytes)?;

    // Verify algorithm
    if header.alg != Algorithm::EdDSA {
        return Err("Invalid algorithm".into());
    }

    // Decode claims
    let claims_bytes = URL_SAFE_NO_PAD.decode(parts[1])?;
    let claims: Claims = serde_json::from_slice(&claims_bytes)?;

    // Verify expiration
    let now = Utc::now().timestamp();
    if claims.exp < now {
        return Err("Token expired".into());
    }

    // Decode signature
    let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2])?;
    let signature = Signature::from_bytes(&signature_bytes.try_into().map_err(|_| "Invalid signature length")?);

    // Verify signature
    let message = format!("{}.{}", parts[0], parts[1]);
    verifying_key.verify(message.as_bytes(), &signature)
        .map_err(|_| "Signature verification failed")?;

    Ok(claims)
}


fn main() {
    println!("=== Ed25519 JWT Demo ===\n");

    // Generate keypair
    println!("1. Generating Ed25519 keypair...");
    let (signing_key, verifying_key) = generate_keypair();

    // Display public key
    let public_key_bytes = verifying_key.to_bytes();
    println!("   Public Key (hex): {}", hex::encode(&public_key_bytes));
    println!("   Public Key (base64): {}\n", URL_SAFE_NO_PAD.encode(&public_key_bytes));

    // Create claims
    println!("2. Creating JWT claims...");
    let now = Utc::now();
    let claims = Claims {
        sub: "user123".to_string(),
        exp: (now + Duration::hours(1)).timestamp(),
        iat: now.timestamp(),
        custom_data: "This is some custom data".to_string(),
    };
    println!("   Subject: {}", claims.sub);
    println!("   Issued At: {} ({})", claims.iat, now.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("   Expires: {} ({})", claims.exp, (now + Duration::hours(1)).format("%Y-%m-%d %H:%M:%S UTC"));
    println!("   Custom Data: {}\n", claims.custom_data);

    // Create and sign JWT
    println!("3. Signing JWT with private key...");
    match create_signed_jwt(&signing_key, &claims) {
        Ok(jwt) => {
            println!("   JWT created successfully!");
            println!("   Token: {}\n", jwt);

            // Verify JWT
            println!("4. Verifying JWT with public key...");
            match verify_jwt(&jwt, &verifying_key) {
                Ok(verified_claims) => {
                    println!("   ✓ JWT verification successful!");
                    println!("   Verified claims:");
                    println!("     - Subject: {}", verified_claims.sub);
                    println!("     - Custom Data: {}", verified_claims.custom_data);
                    println!("     - Issued At: {}", verified_claims.iat);
                    println!("     - Expires: {}\n", verified_claims.exp);
                }
                Err(e) => {
                    println!("   ✗ JWT verification failed: {}", e);
                }
            }

            // Test with wrong key
            println!("5. Testing verification with wrong public key...");
            let (_, wrong_verifying_key) = generate_keypair();
            match verify_jwt(&jwt, &wrong_verifying_key) {
                Ok(_) => {
                    println!("   ✗ Unexpected: JWT verified with wrong key!");
                }
                Err(e) => {
                    println!("   ✓ Expected failure: {}", e);
                }
            }

            // Test with tampered JWT
            println!("\n6. Testing verification with tampered JWT...");
            let mut tampered_jwt = jwt.clone();
            tampered_jwt.push_str("x");
            match verify_jwt(&tampered_jwt, &verifying_key) {
                Ok(_) => {
                    println!("   ✗ Unexpected: Tampered JWT was verified!");
                }
                Err(e) => {
                    println!("   ✓ Expected failure: {}", e);
                }
            }
        }
        Err(e) => {
            println!("   ✗ Failed to create JWT: {}", e);
        }
    }

    println!("\n=== Demo Complete ===");
}

// Add hex dependency for displaying keys
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|byte| format!("{:02x}", byte))
            .collect()
    }
}
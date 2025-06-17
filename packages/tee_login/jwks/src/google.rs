use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct GoogleClaims {
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    pub hd: String,
    pub aud: String,
    pub email: Option<String>,
    // ... other fields as needed
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct GoogleAccount {
    pub nonce: u64,      // iat
    pub address: String, // sub
    pub email: Option<String>,
}

// Keep a map of tokens we've already seen with their expiration times (to prevent reuse).
static USED_TOKENS: Lazy<Mutex<HashMap<String, u64>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// Function to purge expired tokens from the used tokens cache
fn purge_expired_tokens(used_tokens: &mut HashMap<String, u64>, current_time: u64) {
    used_tokens.retain(|_token, &mut exp_time| exp_time > current_time);
}

// Function to decode JWT without verification for debugging
pub fn decode_jwt_for_debug(token: &str) -> Result<Value, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format".to_string());
    }

    // Decode the payload (second part)
    let payload = parts[1];

    // Decode base64 (URL_SAFE_NO_PAD handles missing padding automatically)
    use base64::{Engine as _, engine::general_purpose};
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Parse JSON
    let claims: Value =
        serde_json::from_slice(&decoded).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    Ok(claims)
}

pub fn verify_google_jwt(token: &str, jwks: &Value) -> Result<GoogleAccount, String> {
    let expected_hd = "silvana.one";
    let expected_aud = "1008022345010-m1088p61mjk6d6d8eguebe8pv3vt6psj.apps.googleusercontent.com";

    // 1. Decode header to find which key (kid) was used.
    let header = decode_header(token).map_err(|e| format!("Invalid JWT header: {}", e))?;
    let kid = header.kid.ok_or("Token header missing 'kid'")?;

    // 2. Parse the JWKS and find the JWK matching this kid.
    let jwk_set: JwkSet = serde_json::from_value(jwks.clone())
        .map_err(|e| format!("Failed to parse JWKS JSON: {}", e))?;
    let jwk = jwk_set
        .find(&kid)
        .ok_or("No matching JWK found for token")?;

    // 3. Create a DecodingKey from the JWK (RSA public key).
    let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| format!("JWK error: {}", e))?;

    // 4. Set up validation: must use RS256 and check issuer and audience.
    let mut validation = Validation::new(Algorithm::RS256);
    // Google allows iss = "accounts.google.com" or "https://accounts.google.com" [oai_citation:9‡developers.google.com](https://developers.google.com/identity/gsi/web/guides/verify-google-id-token#:~:text=,token%20represents%20a%20Google%20Workspace).
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    // Ensure 'aud' matches our expected client ID.
    validation.set_audience(&[expected_aud]);
    validation.set_required_spec_claims(&["hd", "aud"]);

    // (We will check expiration manually below, so disable automatic exp check.)
    validation.validate_exp = false;

    // 5. Decode and verify signature.
    let token_data = decode::<GoogleClaims>(token, &decoding_key, &validation)
        .map_err(|e| format!("Token decode/verify failed: {}", e))?;
    let claims = token_data.claims;

    // 6. Check expiration.
    let now = chrono::Utc::now().timestamp() as u64;
    if claims.exp < now {
        return Err("Token has expired".into());
    }

    // 7. Enforce a maximum TTL: Google ID tokens typically have 1 hour (3600s) TTL.
    let ttl = claims
        .exp
        .checked_sub(claims.iat)
        .ok_or("Invalid token times")?;
    if ttl > 3600 {
        return Err(format!("Token TTL too long ({}s)", ttl));
    }

    // 8. Check for required claim: email must be present.
    if claims
        .email
        .as_ref()
        .map(String::as_str)
        .unwrap_or("")
        .is_empty()
    {
        return Err("Token missing required 'email' claim".into());
    }

    // 9. Prevent token reuse: if we've seen this exact token before, reject.
    let mut used = USED_TOKENS.lock().unwrap();

    if claims.hd != expected_hd {
        return Err("Token has invalid 'hd' claim".into());
    }
    if claims.aud != expected_aud {
        return Err("Token has invalid 'aud' claim".into());
    }

    // purge expired tokens to keep memory usage bounded
    purge_expired_tokens(&mut used, now);

    // Check if this specific token has been used before
    if used.contains_key(token) {
        return Err("Token has already been used".into());
    }

    // Store the token with its expiration time
    used.insert(token.to_string(), claims.exp);

    // 10. Return the validated claims on success.
    Ok(GoogleAccount {
        nonce: claims.iat * 1000, // convert to milliseconds
        address: claims.sub,
        email: claims.email,
    })
}

use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    aud: String,
    sub: String,
    exp: i64,
    iat: i64,
}

pub fn generate_jwt_token(secret: &str, audience: &str, user: &str) -> Result<String> {
    let now = Utc::now();
    let expiration = now + Duration::hours(24);

    let claims = Claims {
        aud: audience.to_string(),
        sub: user.to_string(),
        exp: expiration.timestamp(),
        iat: now.timestamp(),
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}
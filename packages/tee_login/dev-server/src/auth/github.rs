use crate::login::VerifyResult;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

pub async fn verify_signature(
    _address: &str,
    signature: &str,
    _message: &str,
) -> Result<VerifyResult, Box<dyn std::error::Error>> {
    // Input validation
    if signature.is_empty() {
        return Ok(VerifyResult {
            is_valid: false,
            address: None,
            nonce: None,
            error: Some("Signature cannot be empty".to_string()),
        });
    }

    let client_id = match std::env::var("GITHUB_CLIENT_ID") {
        Ok(id) => {
            if id.is_empty() {
                return Ok(VerifyResult {
                    is_valid: false,
                    address: None,
                    nonce: None,
                    error: Some("GITHUB_CLIENT_ID environment variable is empty".to_string()),
                });
            }
            id
        }
        Err(_) => {
            return Ok(VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some("GITHUB_CLIENT_ID environment variable not found".to_string()),
            });
        }
    };

    let client_secret = match std::env::var("GITHUB_CLIENT_SECRET") {
        Ok(secret) => {
            if secret.is_empty() {
                return Ok(VerifyResult {
                    is_valid: false,
                    address: None,
                    nonce: None,
                    error: Some("GITHUB_CLIENT_SECRET environment variable is empty".to_string()),
                });
            }
            secret
        }
        Err(_) => {
            return Ok(VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some("GITHUB_CLIENT_SECRET environment variable not found".to_string()),
            });
        }
    };

    let account = match get_github_account(&signature, &client_id, &client_secret).await {
        Ok(account) => account,
        Err(e) => {
            return Ok(VerifyResult {
                is_valid: false,
                address: None,
                nonce: None,
                error: Some(format!("Failed to get GitHub account: {}", e)),
            });
        }
    };

    Ok(VerifyResult {
        is_valid: true,
        address: Some(account.address),
        nonce: Some(account.nonce),
        error: None,
    })
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
struct GitHubUserMeta {
    login: String,
    id: u64,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
struct GitHubTokenMeta {
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
pub struct GitHubAccount {
    pub address: String,
    pub nonce: u64,
    pub name: Option<String>,
    pub email: Option<String>,
}

pub async fn get_github_account(
    access_token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<GitHubAccount, Box<dyn std::error::Error>> {
    // Input validation
    if access_token.is_empty() {
        return Err("Access token cannot be empty".into());
    }

    if client_id.is_empty() {
        return Err("Client ID cannot be empty".into());
    }

    if client_secret.is_empty() {
        return Err("Client secret cannot be empty".into());
    }

    println!("access_token: {}", access_token);

    let user_url = "https://api.github.com/user";
    let token_url = format!("https://api.github.com/applications/{}/token", client_id);

    let client = Client::builder()
        .user_agent("silvana-tee-login/0.1.0")
        .build()?;

    let token_meta = client
        .post(token_url)
        .basic_auth(client_id, Some(client_secret)) // required
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "access_token": access_token }))
        .send()
        .await?
        .error_for_status()? // 404 → invalid / revoked
        .json::<GitHubTokenMeta>()
        .await?;

    let user_meta = client
        .get(user_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()? // 401 → invalid token
        .json::<GitHubUserMeta>()
        .await?;

    // Safe timestamp conversion with overflow protection
    let timestamp = token_meta.created_at.timestamp();
    if timestamp < 0 {
        return Err("Invalid timestamp: cannot be negative".into());
    }

    let nonce = timestamp as u64;

    // Validate user ID
    if user_meta.id == 0 {
        return Err("Invalid user ID: cannot be zero".into());
    }

    let account = GitHubAccount {
        address: user_meta.id.to_string(),
        nonce,
        name: user_meta.name,
        email: user_meta.email,
    };

    Ok(account)
}

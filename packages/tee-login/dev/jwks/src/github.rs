use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

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
) -> Result<GitHubAccount, reqwest::Error> {
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

    let account = GitHubAccount {
        address: user_meta.id.to_string(),
        nonce: token_meta.created_at.timestamp() as u64,
        name: user_meta.name,
        email: user_meta.email,
    };

    Ok(account)
}

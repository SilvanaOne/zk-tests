use reqwest;
use serde_json::Value;
//use std::fs;

pub async fn download_jwks() -> Result<Value, Box<dyn std::error::Error>> {
    // Fetch the JWKS from Google.
    let url = "https://www.googleapis.com/oauth2/v3/certs";
    let resp = reqwest::get(url).await?;
    let jwks_json: Value = resp.json().await?;

    // fs::write("google_jwks.json", serde_json::to_vec_pretty(&jwks_json)?)?;
    // println!("Downloaded JWKS and saved to google_jwks.json");

    // println!("const GOOGLE_JWKS_JSON: &str = r#\"{}\"#;", jwks_json);

    Ok(jwks_json)
}

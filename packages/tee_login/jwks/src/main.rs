mod download;
mod github;
mod google;

use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let test = "github";

    println!("Running test: {}", test);

    match test {
        "github" => github().await?,
        "google" => google().await?,
        _ => {
            println!("Invalid test");
        }
    }

    Ok(())
}

async fn github() -> Result<(), Box<dyn std::error::Error>> {
    let client_id = std::env::var("GITHUB_CLIENT_ID")?;
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET")?;
    let token = std::env::var("GITHUB_TOKEN")?;
    let account = github::get_github_account(&token, &client_id, &client_secret).await?;
    println!("Account: {:?}", account);
    Ok(())
}

async fn google() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("JWT")?;
    //let email = std::env::var("EMAIL")?;

    let jwks = download::download_jwks().await?;

    match google::decode_jwt_for_debug(&token) {
        Ok(claims) => {
            println!(
                "Decoded claims: {:#}",
                serde_json::to_string_pretty(&claims)?
            );
        }
        Err(e) => {
            println!("Failed to decode JWT: {}", e);
        }
    }

    let claims = google::verify_google_jwt(&token, &jwks)?;
    println!("Claims: {:?}", claims);
    Ok(())
}

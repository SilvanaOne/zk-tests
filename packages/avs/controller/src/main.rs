mod agent;
mod coordination;
mod docker;
mod fargate;
mod layers;
use bollard::Docker;
use dotenv::dotenv;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file
    dotenv().ok();
    // Get key from environment
    let key = std::env::var("SUI_KEY").expect("SUI_KEY must be set in .env file");
    coordination::check_gas_coins(&key, true, false).await?;
    let last_request = coordination::get_request().await?;
    let docker = Docker::connect_with_local_defaults()?;
    let mut last_nonce = last_request.nonce;
    loop {
        let new_nonce = agent::agent(&key, &docker, last_nonce).await?;
        if new_nonce > last_nonce {
            println!("New nonce: {:?}", new_nonce);
            last_nonce = new_nonce;
        }
        sleep(Duration::from_secs(5)).await;
    }
}

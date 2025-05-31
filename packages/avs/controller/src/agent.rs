use crate::coordination;
use crate::docker::{load_container, run_container};
use bollard::Docker;
use std::time::Instant;
use sui_sdk::SuiClient;

pub async fn agent(
    sui_client: &SuiClient,
    docker: &Docker,
    last_nonce: u64,
) -> Result<u64, Box<dyn std::error::Error>> {
    let request = coordination::get_request(sui_client).await?;

    if request.nonce <= last_nonce {
        return Ok(last_nonce);
    }
    println!("Nonce: {:?}", request.nonce);
    println!("Agent: {:?}", request.agent);
    println!("Action: {:?}", request.action);
    println!("Request: {:?}", request.request);
    println!("Starting the agent...");
    let time_start = Instant::now();

    // Parameters for container loading
    let use_local_image = true; // Set to false to use Docker Hub
    let image_source = "../agent/out/testagent2.tar.gz";
    //let image_source = format!("dfstio/{}:latest", request.agent);
    //let image_name = format!("{}:latest", request.agent);
    let image_name = "dfstio/testagent2:latest";

    // Load the container image
    if let Err(e) = load_container(docker, use_local_image, &image_source, &image_name).await {
        println!("Failed to load container: {}", e);
        // Return last_nonce to continue processing next request
        return Ok(request.nonce);
    }
    let time_loaded = Instant::now();
    let duration = time_loaded.duration_since(time_start);
    println!("Container loaded in {:?}", duration);

    // Get key from environment
    let key = std::env::var("SUI_KEY").expect("SUI_KEY must be set in .env file");

    // Run container with 30 second timeout
    println!("Running container with 30 second timeout...");
    run_container(
        docker,
        &image_name,
        &key,
        &request.agent,
        &request.action,
        30,
    )
    .await?;

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    println!("Total time taken: {:?}", duration);

    Ok(request.nonce)
}

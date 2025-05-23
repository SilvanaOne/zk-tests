use crate::coordination;
use crate::docker::{load_container, run_container};
use bollard::Docker;
use std::time::Instant;
use tokio::time::{Duration, sleep};

pub async fn start_agent(key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let last_request = coordination::get_request().await?;

    let mut last_nonce = last_request.nonce;
    let docker = Docker::connect_with_local_defaults()?;
    loop {
        let new_nonce = agent(&docker, last_nonce, key).await?;
        if new_nonce > last_nonce {
            println!("New nonce: {:?}", new_nonce);
            last_nonce = new_nonce;
        }
        sleep(Duration::from_secs(5)).await;
    }
}

async fn agent(
    docker: &Docker,
    last_nonce: u64,
    key: &str,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let request = coordination::get_request().await?;

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
    let image_source = format!("dfstio/{}:latest", request.agent);
    let image_name = format!("{}:latest", request.agent);

    // Load the container image using containerd
    if let Err(e) = load_container(
        docker,
        false, // use_local_image parameter set to false (pull from registry)
        &image_source,
        &image_name,
    )
    .await
    {
        println!("Failed to load container: {}", e);
        // Return last_nonce to continue processing next request
        return Ok(request.nonce);
    }
    let time_loaded = Instant::now();
    let duration = time_loaded.duration_since(time_start);
    println!("Container loaded in {:?}", duration);

    // Run container with 900 second timeout
    println!("Running container with 900 second timeout...");
    if let Err(e) = run_container(
        docker,
        &image_name,
        key,
        &request.agent,
        &request.action,
        900,
    )
    .await
    {
        println!("Failed to run container: {}", e);
        return Ok(request.nonce);
    }

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    println!("Total time taken: {:?}", duration);

    Ok(request.nonce)
}

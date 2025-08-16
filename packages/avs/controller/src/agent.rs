use crate::coordination;
use crate::docker::{load_container, run_container};
use crate::fargate::{load_fargate_config_from_env, run_container_fargate};
use aws_config;
use aws_sdk_cloudwatchlogs::Client as LogsClient;
use aws_sdk_ecs::Client as EcsClient;
use bollard::Docker;
use coordination::reply_to_request;
use futures::future;
use std::time::Instant;

pub async fn agent(
    key: &str,
    docker: &Docker,
    last_nonce: u64,
) -> Result<u64, Box<dyn std::error::Error>> {
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

    // Number of parallel requests to run
    let num_parallel_requests = 1;

    // Create futures for all parallel requests
    let mut futures = Vec::new();
    for _ in 0..num_parallel_requests {
        futures.push(reply_to_request(
            &key,
            &request.agent,
            &request.action,
            &request.request,
            5_000_000,
        ));
    }

    // Run all requests in parallel
    let results = futures::future::join_all(futures).await;

    // Check all results for errors
    for result in results {
        result?;
    }
    println!("Executed in {:?} ms", time_start.elapsed().as_millis());
    return Ok(request.nonce);

    // Parameters for container loading
    let use_local_image = false; // Set to false to use Docker Hub
    //let image_source = "../agent/out/testagent2.tar.gz";
    let image_source = format!("dfstio/{}:latest", request.agent);
    let image_name = format!("dfstio/{}:latest", request.agent);
    //let image_name = "dfstio/testagent2:latest";

    // Load AWS configuration
    let config = aws_config::load_from_env().await;
    let ecs_client = EcsClient::new(&config);
    let logs_client = LogsClient::new(&config);
    // Load Fargate configuration from environment variables
    let fargate_config = load_fargate_config_from_env()?;

    // Run container in Fargate instead of local Docker
    run_container_fargate(
        &ecs_client,
        &logs_client,
        &fargate_config,
        &image_name,
        &key,
        &request.agent,
        &request.action,
        300,
    )
    .await?;

    return Ok(request.nonce);

    // Load the container image
    let digest = match load_container(docker, use_local_image, &image_source, &image_name).await {
        Ok(digest) => {
            println!("Container loaded successfully with digest: {}", digest);
            digest
        }
        Err(e) => {
            println!("Failed to load container: {}", e);
            // Return last_nonce to continue processing next request
            return Ok(request.nonce);
        }
    };
    println!("Digest: {}", digest);
    let time_loaded = Instant::now();
    let duration = time_loaded.duration_since(time_start);
    println!("Container loaded in {:?}", duration);

    // Run container with 30 second timeout
    println!("Running container with 900 second timeout...");
    run_container(
        docker,
        &image_name,
        &key,
        &request.agent,
        &request.action,
        900,
    )
    .await?;

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    println!("Total time taken: {:?}", duration);

    Ok(request.nonce)
}

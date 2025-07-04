use crate::coordination;
use crate::docker::{load_container, run_container};
use bollard::Docker;
use std::time::Instant;
use tokio::time::{Duration, sleep};

pub async fn start_agent(key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let last_request = coordination::get_request().await?;

    let mut last_nonce = last_request.nonce;
    let docker = Docker::connect_with_local_defaults()?;
    println!("Docker connected, starting agent...");
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
    let mem_info = sys_info::mem_info();
    println!("mem_info: {:?}", mem_info);
    let time_start = Instant::now();

    // Parameters for container loading
    let image_source = format!("dfstio/{}:flat-amd64", request.agent);
    let image_name = format!("{}:flat-amd64", request.agent);

    // let image_source = "./agents/testagent2.tar.gz";
    // let image_name = "dfstio/testagent2:latest";

    // Load the container image using containerd
    let digest = match load_container(
        docker,
        false, // use_local_image parameter set to false (pull from registry)
        &image_source,
        &image_name,
    )
    .await
    {
        Ok(digest) => {
            println!("Container loaded successfully with digest: {}", digest);
            digest
        },
        Err(e) => {
            println!("Failed to load container: {}", e);
            // Return last_nonce to continue processing next request
            return Ok(request.nonce);
        }
    };
    let time_loaded = Instant::now();
    let duration = time_loaded.duration_since(time_start);
    println!("Container loaded in {:?}", duration);
    let mem_info_loaded = sys_info::mem_info();
    println!("mem_info_loaded: {:?}", mem_info_loaded);

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
    let mem_info_executed = sys_info::mem_info();
    println!("mem_info_executed: {:?}", mem_info_executed);

    Ok(request.nonce)
}

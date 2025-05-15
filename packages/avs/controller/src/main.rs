use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::ImportImageOptions;
use bollard::models::{HostConfig, PortBinding};
use bytes::Bytes;
use futures_util::stream::TryStreamExt;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::{path::Path, time::Instant};
use tokio::time::{self, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting the agent...");
    let time_start_loading = Instant::now();

    // Connect to Docker daemon
    let docker = Docker::connect_with_local_defaults()?;

    // Load the container image
    load_container(&docker).await?;

    // Run container with 30 second timeout
    println!("Running container with 30 second timeout...");
    run_container(&docker, 30).await?;

    // Run container with 90 second timeout
    println!("Running container with 90 second timeout...");
    run_container(&docker, 90).await?;

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start_loading);
    println!("Total time taken: {:?}", duration);

    Ok(())
}

/// Load Docker image from tar file
async fn load_container(docker: &Docker) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading Docker image from tar file...");
    let tar_path = Path::new("../agent/out/app-image.tar");
    if !tar_path.exists() {
        return Err(format!("Image file not found at: {}", tar_path.display()).into());
    }
    let mut file = File::open(tar_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let bytes = Bytes::from(buffer);

    let mut import_stream =
        docker.import_image(ImportImageOptions { quiet: false }, bytes.into(), None);
    while let Some(progress) = import_stream.try_next().await? {
        if let Some(status) = progress.status {
            println!("{}", status);
        }
    }
    println!("Image loaded successfully");

    Ok(())
}

/// Creates, runs and monitors a container with the specified timeout
async fn run_container(
    docker: &Docker,
    timeout_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    // Create and start a container using Bollard with proper port mapping
    println!("Creating and starting container...");
    let container_name = format!("app-container-{}", chrono::Utc::now().timestamp());
    let host_config = HostConfig {
        port_bindings: Some(HashMap::from([(
            "6000/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("6000".to_string()),
            }]),
        )])),
        ..Default::default()
    };
    let config = Config {
        image: Some("app-image:latest"),
        cmd: Some(vec!["npm", "run", "start", "arg-1", "arg-2"]),
        host_config: Some(host_config),
        ..Default::default()
    };
    let create_opts = CreateContainerOptions {
        name: &container_name,
        platform: None,
    };
    let container = docker.create_container(Some(create_opts), config).await?;
    docker
        .start_container(&container.id, None::<StartContainerOptions<String>>)
        .await?;
    println!("Container started successfully with ID: {}", container.id);

    // Record the time it took to start the container
    let container_start_time = start_time.elapsed();
    println!("Container startup time: {:?}", container_start_time);

    // Wait for container to finish or timeout
    println!(
        "Waiting for container to complete (max {} seconds)...",
        timeout_seconds
    );
    let container_id = container.id.clone();

    match time::timeout(
        Duration::from_secs(timeout_seconds),
        monitor_container(docker, &container_id),
    )
    .await
    {
        Ok(result) => {
            result?; // Propagate any error from the monitoring
            println!("Container completed successfully");
        }
        Err(_) => {
            println!(
                "Container took too long (>{} sec), stopping it...",
                timeout_seconds
            );
            docker.stop_container(&container_id, None).await?;
            println!("Container stopped");
        }
    }

    let container_runtime = start_time.elapsed();
    println!("Container ran for: {:?}", container_runtime);

    Ok(())
}

/// Monitor a container until it exits
async fn monitor_container(
    docker: &Docker,
    container_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut status_stream = docker.wait_container::<String>(container_id, None);

    if let Some(status) = status_stream.try_next().await? {
        println!("Container exited with code: {}", status.status_code);
    }

    Ok(())
}

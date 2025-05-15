use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::{CreateImageOptions, ImportImageOptions};
use bollard::models::{HostConfig, PortBinding};
use bytes::Bytes;
use dotenv::dotenv;
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

    // Load .env file
    dotenv().ok();
    let key = std::env::var("SUI_KEY").expect("SUI_KEY must be set in .env file");

    // Connect to Docker daemon
    let docker = Docker::connect_with_local_defaults()?;

    // Parameters for container loading
    let use_local_image = false; // Set to false to use Docker Hub
    //let image_source = "../agent/out/testagent1.tar"; // Path or registry image name (e.g. "nginx:latest" for Docker Hub)
    let image_source_2 = "dfstio/testagent2:latest"; // Name for the loaded image
    let image_name_2 = "testagent2:latest"; // Name for the loaded image
    let image_source_3 = "dfstio/testagent3:latest"; // Name for the loaded image
    let image_name_3 = "testagent3:latest"; // Name for the loaded image

    // Load the container image
    load_container(&docker, use_local_image, image_source_2, image_name_2).await?;

    // Run container with 30 second timeout
    println!("Running container with 30 second timeout...");
    run_container(&docker, image_name_2, &key, "testagent2", "test2", 30).await?;

    load_container(&docker, use_local_image, image_source_3, image_name_3).await?;
    // Run container with 90 second timeout
    println!("Running container with 90 second timeout...");
    run_container(&docker, image_name_3, &key, "testagent3", "test3", 90).await?;

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start_loading);
    println!("Total time taken: {:?}", duration);

    Ok(())
}

/// Load Docker image from tar file or registry
async fn load_container(
    docker: &Docker,
    use_local_image: bool,
    image_source: &str,
    image_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if use_local_image {
        println!("Loading Docker image from local tar file: {}", image_source);
        let tar_path = Path::new(image_source);
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
    } else {
        println!("Pulling Docker image from registry: {}", image_source);

        // Create options for pulling the image
        let options = CreateImageOptions::<String> {
            from_image: image_source.to_string(),
            ..Default::default()
        };

        // Pull the image from the registry
        let mut pull_stream = docker.create_image(Some(options), None, None);

        while let Some(progress) = pull_stream.try_next().await? {
            if let Some(status) = progress.status {
                println!("{}", status);
                // if let Some(progress) = progress.progress {
                //     //println!("  {}", progress);
                // }
            }
        }

        // Tag the image if needed
        if image_source != image_name {
            println!("Tagging image as: {}", image_name);
            docker
                .tag_image(
                    image_source,
                    Some(bollard::image::TagImageOptions {
                        repo: image_name.split(':').next().unwrap_or(image_name),
                        tag: image_name.split(':').nth(1).unwrap_or("latest"),
                    }),
                )
                .await?;
        }

        println!("Image pulled successfully");
    }

    Ok(())
}

/// Creates, runs and monitors a container with the specified timeout
async fn run_container(
    docker: &Docker,
    image_name: &str,
    key: &str,
    agent: &str,
    action: &str,
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
        image: Some(image_name),
        cmd: Some(vec!["npm", "run", "start", key, agent, action]),
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

use bollard::Docker;
use bollard::models::ContainerCreateBody;
use bollard::models::{HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ImportImageOptions, ListImagesOptions, LogsOptions,
    StartContainerOptions, StopContainerOptions, TagImageOptions, WaitContainerOptions,
};
use bytes::Bytes;
use futures_util::stream::TryStreamExt;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::{path::Path, time::Instant};
use tokio::time::{self, Duration};

/// Load Docker image from tar file or registry
pub async fn load_container(
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

        let options = ImportImageOptions {
            quiet: false,
            platform: None,
        };

        let mut import_stream = docker.import_image(options, bollard::body_full(bytes), None);
        while let Some(progress) = import_stream.try_next().await? {
            if let Some(status) = progress.status {
                println!("{}", status);
            }
        }

        println!("Image loaded successfully");
        let images = docker
            .list_images(Some(ListImagesOptions::default()))
            .await?;
        println!("Images: {:?}", images);
    } else {
        println!("Pulling Docker image from registry: {}", image_source);

        // Create options for pulling the image
        let options = CreateImageOptions {
            from_image: Some(image_source.to_string()),
            from_src: None,
            repo: None,
            tag: None,
            message: None,
            changes: vec![],
            platform: "".to_string(),
        };

        // Pull the image
        let pull_result = async {
            let mut pull_stream = docker.create_image(Some(options), None, None);

            while let Some(progress) = pull_stream.try_next().await? {
                if let Some(status) = progress.status {
                    println!("{}", status);
                }
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;

        if let Err(e) = pull_result {
            // Check if error message contains repository not found indicators
            let err_string = e.to_string();
            if err_string.contains("404") || err_string.contains("repository does not exist") {
                return Err(format!(
                    "Image '{}' not found. Please check the image name or login to the registry.",
                    image_source
                )
                .into());
            }
            return Err(e);
        }
        // Tag the image if needed
        if image_source != image_name {
            println!("Tagging image as: {}", image_name);
            let tag_options = TagImageOptions {
                repo: Some(
                    image_name
                        .split(':')
                        .next()
                        .unwrap_or(image_name)
                        .to_string(),
                ),
                tag: Some(image_name.split(':').nth(1).unwrap_or("latest").to_string()),
            };
            docker.tag_image(image_source, Some(tag_options)).await?;
        }

        println!("Image pulled successfully");
    }

    Ok(())
}

/// Creates, runs and monitors a container with the specified timeout
pub async fn run_container(
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
    let config = ContainerCreateBody {
        image: Some(image_name.to_string()),
        cmd: Some(vec![
            "npm".to_string(),
            "run".to_string(),
            "start".to_string(),
            key.to_string(),
            agent.to_string(),
            action.to_string(),
        ]),
        host_config: Some(host_config),
        ..Default::default()
    };
    let create_opts = CreateContainerOptions {
        name: Some(container_name.clone()),
        platform: "".to_string(),
    };
    let container = docker.create_container(Some(create_opts), config).await?;

    docker
        .start_container(&container.id, Some(StartContainerOptions::default()))
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

            // Get container logs using the correct method
            let logs_options = LogsOptions {
                stdout: true,
                stderr: true,
                since: 0,
                until: 0,
                timestamps: false,
                follow: false,
                tail: "".to_string(),
            };

            let mut logs_stream = docker.logs(&container_id, Some(logs_options));
            let mut all_logs = String::new();

            while let Some(log_result) = logs_stream.try_next().await? {
                match log_result {
                    bollard::container::LogOutput::StdOut { message }
                    | bollard::container::LogOutput::StdErr { message } => {
                        all_logs.push_str(&String::from_utf8_lossy(&message));
                    }
                    _ => {} // Handle other log output variants if needed
                }
            }

            println!("Container logs:\n{}", all_logs);
        }
        Err(_) => {
            println!(
                "Container took too long (>{} sec), stopping it...",
                timeout_seconds
            );
            let stop_options = StopContainerOptions {
                t: Some(30),
                signal: None,
            };
            docker
                .stop_container(&container_id, Some(stop_options))
                .await?;
            println!("Container stopped");
        }
    }

    let container_runtime = start_time.elapsed();
    println!("Container ran for: {:?}", container_runtime);

    Ok(())
}

/// Monitor a container until it exits
pub async fn monitor_container(
    docker: &Docker,
    container_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let wait_options = WaitContainerOptions {
        condition: "not-running".to_string(),
    };
    let mut status_stream = docker.wait_container(container_id, Some(wait_options));

    if let Some(status) = status_stream.try_next().await? {
        println!("Container exited with code: {}", status.status_code);
    }

    Ok(())
}

// use sysinfo::{System, SystemExt};

// // Inside your function:
// let mut system = System::new_all();
// system.refresh_memory();  // Refresh memory information

// let total_memory = system.total_memory();  // Get total memory in KB
// let reserve_for_host = 1_000_000;  // 1GB in KB
// let container_memory_limit = if total_memory > reserve_for_host {
//     (total_memory - reserve_for_host) * 1024  // Convert to bytes for Docker API
// } else {
//     println!("Warning: System has less than 1GB total memory!");
//     total_memory / 2 * 1024  // Use half of available memory as fallback
// };

// // Then use this value in your HostConfig
// let host_config = HostConfig {
//     port_bindings: Some(HashMap::from([(
//         "6000/tcp".to_string(),
//         Some(vec![PortBinding {
//             host_ip: Some("0.0.0.0".to_string()),
//             host_port: Some("6000".to_string()),
//         }]),
//     )])),
//     memory: Some(container_memory_limit),
//     memory_swap: Some(container_memory_limit),  // Disable swap
//     ..Default::default()
// };

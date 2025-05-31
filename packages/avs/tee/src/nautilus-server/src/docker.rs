use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::{CreateImageOptions, ImportImageOptions};
use bollard::models::{HostConfig, PortBinding};
use bytes::Bytes;
use futures_util::stream::TryStreamExt;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::{path::Path, time::Instant};
use tokio::time::{self, Duration};

/// Load Docker image from tar file or registry
pub async fn load_container(
    docker: &Docker,
    use_local_image: bool,
    image_source: &str,
    image_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

        // Pull the image
        let pull_result = async {
            let mut pull_stream = docker.create_image(Some(options), None, None);

            while let Some(progress) = pull_stream.try_next().await? {
                if let Some(status) = progress.status {
                    println!("{}", status);
                }
            }
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
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
        if let Err(e) = drop_page_cache() {
            println!("couldn't drop caches: {}", e);
        }
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    if let Err(e) = drop_page_cache() {
        println!("couldn't drop caches: {}", e);
    }

    Ok(())
}

/// Monitor a container until it exits
pub async fn monitor_container(
    docker: &Docker,
    container_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut status_stream = docker.wait_container::<String>(container_id, None);

    if let Some(status) = status_stream.try_next().await? {
        println!("Container exited with code: {}", status.status_code);
    }

    Ok(())
}



/// Flush filesystem buffers and drop the kernel page-cache, dentries, and inodes.
///
/// Requires the process to run as root (needs `CAP_SYS_ADMIN` to write
/// to `/proc/sys/vm/drop_caches`).
pub fn drop_page_cache() -> std::io::Result<()> {
    // 1. sync(2) – flush dirty pages to disk
    unsafe { libc::sync() };            // single libc call, no return value

    // 2. echo 3 > /proc/sys/vm/drop_caches
    let mut f = OpenOptions::new()
        .write(true)
        .open("/proc/sys/vm/drop_caches")?;
    // Writing just “3” is enough; a trailing '\n' is optional.
    f.write_all(b"3")?;
    // File is closed when `f` goes out of scope
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

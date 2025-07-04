use bollard::Docker;
use bollard::models::ContainerCreateBody;
use bollard::models::HostConfig;
use bollard::query_parameters::InspectContainerOptions;
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ImportImageOptions, ListImagesOptions, LogsOptions,
    PruneContainersOptionsBuilder, PruneImagesOptionsBuilder, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions, TagImageOptions,
};
use bytes::Bytes;
use futures_util::stream::TryStreamExt;
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
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

    // Get the digest of the image
    println!("Getting image digest...");
    let image_inspect = docker.inspect_image(image_name).await?;
    
    // Extract SHA256 digest from the RepoDigests
    let digest = if let Some(repo_digests) = image_inspect.repo_digests {
        if let Some(first_digest) = repo_digests.first() {
            // RepoDigests format is typically: "repo@sha256:digest"
            if let Some(sha_part) = first_digest.split('@').nth(1) {
                sha_part.to_string()
            } else {
                return Err("Unable to extract digest from repo digests".into());
            }
        } else {
            return Err("No repo digests found for the image".into());
        }
    } else if let Some(id) = image_inspect.id {
        // If no repo digests, use the image ID as fallback
        // Image ID format is typically: "sha256:digest"
        if let Some(sha_part) = id.split(':').nth(1) {
            format!("sha256:{}", sha_part)
        } else {
            id
        }
    } else {
        return Err("Unable to determine image digest".into());
    };

    println!("Image digest: {}", digest);
    Ok(digest)
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
        // `--network host` equivalent
        network_mode: Some("host".to_string()),
        // no port bindings needed in host‑network mode
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
    docker
        .remove_container(&container_id, None::<RemoveContainerOptions>)
        .await?;
    docker
        .prune_images(Some(PruneImagesOptionsBuilder::new().build()))
        .await?;
    docker
        .prune_containers(Some(PruneContainersOptionsBuilder::new().build()))
        .await?;
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
    // ---------------------------------------------------------------------
    // In Nitro‑Enclave containerd can drop task‑deleted events, which makes
    // `docker.wait_container` return an empty error.  We fall back to a
    // polling strategy via `docker.inspect_container`.
    // ---------------------------------------------------------------------
    println!("Polling container status…");

    loop {
        // Ask Docker for the latest container state.
        let details = docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await?;

        if let Some(state) = details.state {
            if state.running == Some(false) || state.status.as_ref().map(|s| s.as_ref()) == Some("exited") {
                let exit_code = state.exit_code.unwrap_or_default();
                println!("Container exited with code: {exit_code}");
                if exit_code != 0 {
                    return Err(format!(
                        "Container exited with non‑zero status code: {exit_code}"
                    )
                    .into());
                }
                break;
            }
        }

        // Still running – sleep a bit before the next check.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    //tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    println!("Finished waiting for container to exit");
    Ok(())
}

pub fn drop_page_cache() -> std::io::Result<()> {
    // 1. sync(2) – flush dirty pages to disk
    unsafe { libc::sync() }; // single libc call, no return value

    // 2. echo 3 > /proc/sys/vm/drop_caches
    let mut f = OpenOptions::new()
        .write(true)
        .open("/proc/sys/vm/drop_caches")?;
    // Writing just "3" is enough; a trailing '\n' is optional.
    f.write_all(b"3")?;
    // File is closed when `f` goes out of scope
    Ok(())
}

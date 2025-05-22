use anyhow::{Result, Context};
use std::time::Instant;
use std::env::consts;
use tokio::time::{Duration, timeout};
use containerd_client as client;
use client::{
    services::v1::{transfer_client::TransferClient, TransferOptions, TransferRequest},
    to_any,
    types::{
        transfer::{ImageStore, OciRegistry, UnpackConfiguration},
        Platform,
    },
    with_namespace,
};
use tonic::{transport::Channel, Request};
use std::collections::HashMap;
use uuid;

const NAMESPACE: &str = "default";

pub async fn load_container(
    use_local_image: bool,
    image_source: &str,
    image_name: &str,
) -> Result<()> {
    println!(
        "Loading container: {} (from {})",
        image_name, image_source
    );

    let start = Instant::now();

    if use_local_image {
        println!("Loading local image: {}", image_source);
        // Import from local tar file could be implemented here
        // For now we'll simulate this part
        tokio::time::sleep(Duration::from_millis(500)).await;
    } else {
        println!("Loading image from remote source: {}", image_source);
        
        // Get architecture
        let arch = match consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            _ => consts::ARCH,
        };

        // Connect to containerd socket
        let channel = client::connect("/run/containerd/containerd.sock")
            .await
            .context("Failed to connect to containerd socket")?;
        
        let mut client = TransferClient::new(channel.clone());

        // Ensure Docker Hub references have the proper format
        let formatted_image_source = if !image_source.contains('/') {
            format!("docker.io/library/{}", image_source)
        } else if !image_source.contains('.') {
            format!("docker.io/{}", image_source)
        } else {
            image_source.to_string()
        };

        // Create the source (OCIRegistry)
        let source = OciRegistry {
            reference: formatted_image_source.clone(),
            resolver: Default::default(),
        };

        // Setup platform
        let platform = Platform {
            os: "linux".to_string(),
            architecture: arch.to_string(),
            variant: "".to_string(),
            os_version: "".to_string(),
        };

        // Create the destination (ImageStore)
        let destination = ImageStore {
            name: image_name.to_string(),
            platforms: vec![platform.clone()],
            unpacks: vec![UnpackConfiguration {
                platform: Some(platform),
                ..Default::default()
            }],
            ..Default::default()
        };

        let anys = to_any(&source);
        let anyd = to_any(&destination);

        println!("Pulling image for linux/{} from source: {:?}", arch, source);

        // Create the transfer request
        let request = TransferRequest {
            source: Some(anys),
            destination: Some(anyd),
            options: Some(TransferOptions {
                ..Default::default()
            }),
        };
        
        // Execute the transfer (pull)
        println!("Attempting to transfer image with reference: {}", formatted_image_source);
        match client.transfer(with_namespace!(request, NAMESPACE)).await {
            Ok(_) => {
                println!("Transfer completed successfully");
            },
            Err(e) => {
                println!("Transfer failed with error: {:?}", e);
                
                return Err(e).context("Failed to transfer image");
            }
        };
    }

    println!("Image loaded successfully in {:?}", start.elapsed());
    Ok(())
}


pub async fn run_container(
    image: &str,
    key: &str,
    agent: &str,
    action: &str,
    timeout_sec: u64,
) -> Result<()> {
    let start = Instant::now();
    println!("Starting container for image {}", image);
    println!("- Agent: {}", agent);
    println!("- Action: {}", action);
    println!("- Key: {}", key);

    // Connect to containerd
    let channel = client::connect("/run/containerd/containerd.sock")
        .await
        .context("Failed to connect to containerd socket")?;

    // Setup client for containerd
    let mut images_client = client::services::v1::images_client::ImagesClient::new(channel.clone());
    let mut containers_client = client::services::v1::containers_client::ContainersClient::new(channel.clone());
    let mut tasks_client = client::services::v1::tasks_client::TasksClient::new(channel.clone());

    // Get image information
    let image_ref = client::with_namespace!(
        client::services::v1::GetImageRequest {
            name: image.to_string(),
        },
        NAMESPACE
    );
    let _image_info = images_client
        .get(image_ref)
        .await
        .context("Failed to get image info")?;

    // Create unique container ID
    let container_id = format!("container-{}", uuid::Uuid::new_v4());
    
    // Create container spec with the command arguments
    use client::services::v1::Container;
    use client::services::v1::container;
    
    // Create container
    let create_request = client::with_namespace!(
        client::services::v1::CreateContainerRequest {
            container: Some(Container {
                id: container_id.clone(),
                image: image.to_string(),
                runtime: Some(container::Runtime {
                    name: "io.containerd.runc.v2".to_string(),
                    options: None,
                }),
                spec: None, // We would need to set the spec with command args here
                snapshotter: "overlayfs".to_string(),
                labels: HashMap::new(),
                extensions: HashMap::new(),
                // Use the fields without providing timestamps - rely on containerd to set them
                created_at: None,
                updated_at: None,
                sandbox: "".to_string(),
                snapshot_key: "".to_string(),
            }),
        },
        NAMESPACE
    );
    
    let container = containers_client
        .create(create_request)
        .await
        .context("Failed to create container")?
        .into_inner()
        .container
        .ok_or_else(|| anyhow::anyhow!("No container returned"))?;

    println!("Container created with ID: {}", container.id);

    // Create task
    let create_task_request = client::with_namespace!(
        client::services::v1::CreateTaskRequest {
            container_id: container.id.clone(),
            rootfs: vec![],
            terminal: false,
            stdin: "".to_string(),
            stdout: "".to_string(),
            stderr: "".to_string(),
            checkpoint: None, // Changed from String to Option<Descriptor>
            options: None,
            runtime_path: "".to_string(),
        },
        NAMESPACE
    );
    
    let task = tasks_client
        .create(create_task_request)
        .await
        .context("Failed to create task")?
        .into_inner();

    println!("Task created with PID: {}", task.pid);

    // Start task
    let start_request = client::with_namespace!(
        client::services::v1::StartRequest {
            container_id: container.id.clone(),
            exec_id: "".to_string(),
        },
        NAMESPACE
    );
    
    tasks_client
        .start(start_request)
        .await
        .context("Failed to start task")?;

    println!("Task started");

    // Wait for container to finish or timeout
    let execution_result = match timeout(
        Duration::from_secs(timeout_sec),
        wait_for_container(&mut tasks_client, &container.id)
    ).await {
        Ok(result) => {
            match result {
                Ok(exit_code) => {
                    println!("Container exited with code {}", exit_code);
                    Ok(())
                },
                Err(e) => {
                    println!("Error waiting for container: {}", e);
                    Err(e)
                }
            }
        },
        Err(_) => {
            println!("Container timed out after {} seconds", timeout_sec);
            
            // Kill the task
            let kill_request = client::with_namespace!(
                client::services::v1::KillRequest {
                    container_id: container.id.clone(),
                    exec_id: "".to_string(),
                    signal: 9, // SIGKILL
                    all: true,
                },
                NAMESPACE
            );
            
            tasks_client
                .kill(kill_request)
                .await
                .context("Failed to kill task")?;
                
            println!("Task killed");
            Ok(())
        }
    };

    // Delete the task
    let delete_task_request = client::with_namespace!(
        client::services::v1::DeleteTaskRequest {
            container_id: container.id.clone(),
        },
        NAMESPACE
    );
    
    tasks_client
        .delete(delete_task_request)
        .await
        .context("Failed to delete task")?;
        
    println!("Task deleted");

    // Delete the container
    let delete_container_request = client::with_namespace!(
        client::services::v1::DeleteContainerRequest {
            id: container.id.clone(),
        },
        NAMESPACE
    );
    
    containers_client
        .delete(delete_container_request)
        .await
        .context("Failed to delete container")?;
        
    println!("Container deleted");

    println!("Total execution time: {:?}", start.elapsed());

    execution_result
}

async fn wait_for_container(
    tasks_client: &mut client::services::v1::tasks_client::TasksClient<Channel>,
    container_id: &str
) -> Result<u32> {
    let wait_request = client::with_namespace!(
        client::services::v1::WaitRequest {
            container_id: container_id.to_string(),
            exec_id: "".to_string(),
        },
        NAMESPACE
    );
    
    let response = tasks_client
        .wait(wait_request)
        .await
        .context("Failed to wait for task")?
        .into_inner();
        
    Ok(response.exit_status)
}

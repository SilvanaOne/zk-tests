use anyhow::Result;
use nautilus_server::containerd;

#[tokio::main]
async fn main() -> Result<()> {
    // First pull the image if it's not already loaded
    let image_source = "docker.io/library/alpine:latest";
    let image_name = "alpine:latest";
    
    println!("Loading container image...");
    containerd::load_container(false, image_source, image_name).await?;
    
    // Run the container with a 30 second timeout
    println!("\nRunning container...");
    containerd::run_container(
        image_name,
        "encryption-key-123",  // encryption key
        "test-agent",          // agent name
        "validate",            // action to perform
        30,                    // timeout in seconds
    ).await?;
    
    Ok(())
} 
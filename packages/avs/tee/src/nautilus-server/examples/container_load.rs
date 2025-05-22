use anyhow::Result;
use nautilus_server::containerd;

#[tokio::main]
async fn main() -> Result<()> {
    // Docker Hub image reference
    let image_source = "docker.io/library/alpine:latest";
    let image_name = "alpine:latest";
    
    println!("Demonstrating remote image loading:");
    containerd::load_container(false, image_source, image_name).await?;
    
    println!("\nDemonstrating local image loading simulation:");
    containerd::load_container(true, "/path/to/local/image.tar", image_name).await?;
    
    Ok(())
} 
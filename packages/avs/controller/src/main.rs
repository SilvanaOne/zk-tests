use bollard::Docker;
use std::process::Command;
use std::{path::Path, time::Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting the agent...");
    let time_start = Instant::now();

    // Connect to Docker daemon
    let docker = Docker::connect_with_local_defaults()?;

    // Load image from the tar file using Docker CLI
    println!("Loading Docker image from tar file...");
    let tar_path = Path::new("../agent/out/app-image.tar");
    if !tar_path.exists() {
        return Err(format!("Image file not found at: {}", tar_path.display()).into());
    }

    let output = Command::new("docker")
        .args(["load", "-i", "../agent/out/app-image.tar"])
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to load Docker image: {}", error).into());
    }

    println!("Image loaded successfully");

    // Create and start a container using Docker CLI with proper port mapping
    println!("Creating and starting container...");
    let container_name = format!("app-container-{}", chrono::Utc::now().timestamp());

    let run_output = Command::new("docker")
        .args([
            "run",
            "-d", // Detached mode
            "-p",
            "6000:6000", // Port mapping
            "--name",
            &container_name,
            "app-image:latest",
            "npm",
            "run",
            "start",
            "arg-1",
            "arg-2",
        ])
        .output()?;

    if !run_output.status.success() {
        let error = String::from_utf8_lossy(&run_output.stderr);
        return Err(format!("Failed to run container: {}", error).into());
    }

    let container_id = String::from_utf8_lossy(&run_output.stdout)
        .trim()
        .to_string();
    println!("Container started successfully with ID: {}", container_id);

    let time_end = Instant::now();
    let duration = time_end.duration_since(time_start);
    println!("Time taken: {:?}", duration);

    Ok(())
}

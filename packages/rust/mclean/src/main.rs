use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Default)]
struct CleanupStats {
    node_modules_removed: u32,
    target_dirs_removed: u32,
    total_size_freed: u64,
}

fn main() {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    println!("Starting cleanup in: {}", current_dir.display());
    
    let mut stats = CleanupStats::default();
    if let Err(e) = cleanup_directory(&current_dir, &mut stats) {
        eprintln!("Error during cleanup: {}", e);
        std::process::exit(1);
    }
    
    println!("Cleanup completed successfully!");
    println!("\n=== Cleanup Statistics ===");
    println!("Node.js modules removed: {}", stats.node_modules_removed);
    println!("Rust target directories removed: {}", stats.target_dirs_removed);
    println!("Total space freed: {:.2} MB", stats.total_size_freed as f64 / 1_048_576.0);
}

fn cleanup_directory(dir: &Path, stats: &mut CleanupStats) -> Result<(), Box<dyn std::error::Error>> {
    let entries = fs::read_dir(dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let dir_name = path.file_name().unwrap().to_string_lossy();
            
            // Check if this is a node_modules folder and if package.json exists in parent
            if dir_name == "node_modules" {
                let package_json = dir.join("package.json");
                if package_json.exists() {
                    println!("Removing node_modules: {}", path.display());
                    let size = calculate_dir_size(&path).unwrap_or(0);
                    if let Err(e) = fs::remove_dir_all(&path) {
                        eprintln!("Warning: Failed to remove {}: {}. Trying alternative method...", path.display(), e);
                        if let Err(e2) = force_remove_dir(&path) {
                            eprintln!("Failed to remove {}: {}", path.display(), e2);
                            continue;
                        }
                    }
                    stats.node_modules_removed += 1;
                    stats.total_size_freed += size;
                    continue;
                }
            }
            
            // Check if this is a target folder and if Cargo.toml exists in parent
            if dir_name == "target" {
                let cargo_toml = dir.join("Cargo.toml");
                if cargo_toml.exists() {
                    println!("Removing target: {}", path.display());
                    let size = calculate_dir_size(&path).unwrap_or(0);
                    if let Err(e) = fs::remove_dir_all(&path) {
                        eprintln!("Warning: Failed to remove {}: {}. Trying alternative method...", path.display(), e);
                        if let Err(e2) = force_remove_dir(&path) {
                            eprintln!("Failed to remove {}: {}", path.display(), e2);
                            continue;
                        }
                    }
                    stats.target_dirs_removed += 1;
                    stats.total_size_freed += size;
                    continue;
                }
            }
            
            // Recursively process subdirectories
            cleanup_directory(&path, stats)?;
        }
    }
    
    Ok(())
}

fn calculate_dir_size(dir: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut size = 0;
    let entries = fs::read_dir(dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            size += entry.metadata()?.len();
        } else if path.is_dir() {
            size += calculate_dir_size(&path)?;
        }
    }
    
    Ok(size)
}

fn force_remove_dir(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("rm")
        .arg("-rf")
        .arg(dir)
        .output()?;
    
    if !output.status.success() {
        return Err(format!("rm command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    Ok(())
}

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Building Sui gRPC client from proto files...");

    // Find all proto files in the proto directory
    let proto_files = find_proto_files("proto")?;

    println!("📁 Found {} proto files:", proto_files.len());
    for proto_file in &proto_files {
        println!("   - {}", proto_file.display());
    }

    // Use standard tonic-build configuration
    tonic_build::configure()
        //.disable_package_emission()
        //.out_dir("src/proto")
        .build_server(false) // We only need the client
        .build_client(true) // Generate client code
        .compile_well_known_types(false) // Generate all types from proto files
        .emit_rerun_if_changed(false) // We'll handle this manually
        .compile_protos(
            &proto_files
                .iter()
                .map(|p| p.to_str().unwrap())
                .collect::<Vec<_>>(),
            &["proto"],
        )
        .map_err(|e| {
            eprintln!("❌ Failed to compile proto files: {}", e);
            eprintln!("Error details: {:?}", e);
            e
        })?;

    // Post-process generated files to fix module references
    fix_generated_module_references()?;

    // Set up rerun-if-changed for all proto files
    for proto_file in &proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }

    // Also rerun if the proto directory structure changes
    println!("cargo:rerun-if-changed=proto/");

    println!("✅ Successfully compiled {} proto files", proto_files.len());
    println!("🎯 gRPC client code generated");

    Ok(())
}

/// Fix module references in generated proto files
fn fix_generated_module_references() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let generated_file = format!("{}/sui.rpc.v2beta2.rs", out_dir);

    // Check if the generated file exists
    if !std::path::Path::new(&generated_file).exists() {
        println!("⚠️  Generated file not found: {}", generated_file);
        return Ok(());
    }

    // Read the generated file
    let content = std::fs::read_to_string(&generated_file)?;

    // Fix the problematic super:: references
    let fixed_content = content.replace(
        "super::super::super::super::google::rpc::Status",
        "crate::proto::google::rpc::Status",
    );

    // Only write back if there were changes
    if content != fixed_content {
        std::fs::write(&generated_file, fixed_content)?;
        println!("🔧 Fixed module references in generated proto file");
    }

    Ok(())
}

/// Recursively find all .proto files in the given directory
fn find_proto_files(dir: &str) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut proto_files = Vec::new();
    let proto_dir = PathBuf::from(dir);

    if !proto_dir.exists() {
        return Err(format!("Proto directory '{}' does not exist", dir).into());
    }

    collect_proto_files(&proto_dir, &mut proto_files)?;

    // Sort for consistent build order
    proto_files.sort();

    Ok(proto_files)
}

/// Recursively collect all .proto files from a directory
fn collect_proto_files(
    dir: &PathBuf,
    proto_files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recursively search subdirectories
            collect_proto_files(&path, proto_files)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("proto") {
            // Add .proto files
            proto_files.push(path);
        }
    }

    Ok(())
}

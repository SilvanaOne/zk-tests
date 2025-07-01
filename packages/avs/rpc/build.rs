use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path = PathBuf::from("proto/events_descriptor.bin");

    // Generate protobuf code
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(descriptor_path)
        .compile(&["proto/events.proto"], &["proto"])?;

    // Tell cargo to recompile if any .proto files change
    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}

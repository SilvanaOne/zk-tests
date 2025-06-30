use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate protobuf code
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("events_descriptor.bin"))
        .compile(&["proto/events.proto"], &["proto"])?;

    // Tell cargo to recompile if any .proto files change
    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("proto/kv_descriptor.bin")
        .compile(&["proto/kv.proto"], &["proto"])?;
    Ok(())
}

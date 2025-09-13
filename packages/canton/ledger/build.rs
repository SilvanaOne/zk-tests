use std::path::PathBuf;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the OUT_DIR where we'll generate the files
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Configure tonic-build with protoc
    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional");
    
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(&out_dir)
        .compile_protos_with_config(
            config,
            &[
                "proto/com/daml/ledger/api/v2/version_service.proto",
                "proto/com/daml/ledger/api/v2/state_service.proto",
                "proto/com/daml/ledger/api/v2/update_service.proto",
                "proto/com/daml/ledger/api/v2/command_completion_service.proto",
                "proto/com/daml/ledger/api/v2/package_service.proto",
                "proto/com/daml/ledger/api/v2/admin/user_management_service.proto",
                "proto/com/daml/ledger/api/v2/admin/party_management_service.proto",
            ],
            &["proto"],
        )?;
    
    println!("cargo:rerun-if-changed=proto");
    
    Ok(())
}
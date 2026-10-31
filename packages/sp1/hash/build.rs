use std::process::Command;

use sp1_build::{BuildArgs, build_program_with_args};

fn main() {
    // Guests build inside SP1's Docker image, so no local SP1 toolchain is needed and their ELFs
    // and verifying keys are reproducible.
    build_program_with_args(
        "program",
        BuildArgs {
            docker: true,
            locked: true,
            ..Default::default()
        },
    );

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let version = Command::new(rustc)
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=HOST_RUSTC_VERSION={version}");
}

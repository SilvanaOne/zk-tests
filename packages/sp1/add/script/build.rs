use sp1_build::build_program_with_args;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Build the SP1 programs
    build_program_with_args("../programs/add", Default::default());
    build_program_with_args("../programs/aggregate", Default::default());
    build_program_with_args("../programs/sha256", Default::default());
    build_program_with_args("../programs/p3", Default::default());
    // build_program_with_args("../programs/ps", Default::default());  // Disabled due to mina-hasher serde conflict
    // build_program_with_args("../programs/proof", Default::default());  // Disabled due to proof-lib dependency
    build_program_with_args("../programs/mina", Default::default());
    build_program_with_args("../programs/price", Default::default());

    // Generate ABI from Add.sol
    generate_abi();
}

fn generate_abi() {
    let ethereum_dir = Path::new("../ethereum");
    let abi_dir = Path::new("abi");

    // Create abi directory if it doesn't exist
    if !abi_dir.exists() {
        fs::create_dir_all(abi_dir).expect("Failed to create abi directory");
    }

    // Run forge to generate ABI
    // Skip if forge is not available (e.g., in Docker builds)
    let output = match Command::new("forge")
        .args([
            "build",
            "--extra-output-files",
            "abi",
            "--root",
            ethereum_dir.to_str().unwrap(),
        ])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Warning: forge not found, skipping ABI generation: {}", e);
            return;
        }
    };

    if !output.status.success() {
        eprintln!(
            "Forge build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    // Copy the generated ABI file
    let source_abi = ethereum_dir.join("out/Add.sol/Add.json");
    let dest_abi = abi_dir.join("Add.json");

    if source_abi.exists() {
        // Read the forge output JSON and extract just the ABI
        let forge_output = fs::read_to_string(&source_abi).expect("Failed to read forge output");

        let json: serde_json::Value =
            serde_json::from_str(&forge_output).expect("Failed to parse forge output JSON");

        if let Some(abi) = json.get("abi") {
            // Write just the ABI to the destination file
            fs::write(&dest_abi, serde_json::to_string_pretty(abi).unwrap())
                .expect("Failed to write ABI file");

            println!("Successfully updated {}", dest_abi.display());
        } else {
            eprintln!("No ABI found in forge output");
        }
    } else {
        eprintln!("Forge output file not found: {}", source_abi.display());
    }
}

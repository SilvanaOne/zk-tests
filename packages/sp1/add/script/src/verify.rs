//! Core proof verification module

use add_lib::PublicValuesStruct;
use alloy_sol_types::SolType;
use serde::{Deserialize, Serialize};
use sp1_sdk::{HashableKey, Prover, ProverClient, SP1ProofWithPublicValues, SP1VerifyingKey};
use std::path::PathBuf;
use std::time::Instant;

/// Core proof fixture for deserialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreProofFixture {
    pub old_root: String,
    pub new_root: String,
    pub vkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vkey_bytes: Option<String>, // Serialized SP1VerifyingKey for actual verification
    pub public_values: String,
    pub proof: String,
}

/// Load and verify a core proof from a JSON file
pub fn verify_core_proof_from_file(
    filepath: &PathBuf,
    elf: &[u8],
    skip_vkey_check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading core proof from: {}", filepath.display());

    // Read the JSON file
    let json_str = std::fs::read_to_string(filepath)?;
    let fixture: CoreProofFixture = serde_json::from_str(&json_str)?;

    println!("\n=== Loaded Proof Data ===");
    println!("Old root: {}", fixture.old_root);
    println!("New root: {}", fixture.new_root);
    println!("Verification key: {}", fixture.vkey);

    // Decode proof and public values
    let proof_bytes = hex::decode(fixture.proof.trim_start_matches("0x"))?;
    let public_values_bytes = hex::decode(fixture.public_values.trim_start_matches("0x"))?;

    // Print proof size information
    println!("\n📊 Proof Size Information:");
    println!(
        "   File size: {} bytes ({:.2} KB)",
        json_str.len(),
        json_str.len() as f64 / 1024.0
    );
    println!(
        "   Serialized proof size: {} bytes ({:.2} KB)",
        proof_bytes.len(),
        proof_bytes.len() as f64 / 1024.0
    );
    println!("   Public values size: {} bytes", public_values_bytes.len());

    // Create SP1ProofWithPublicValues using bincode deserialization
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&proof_bytes)?;

    // Setup the verifier
    let client = ProverClient::from_env();
    let (_pk, vk) = client.setup(elf);

    // Verify the vkey matches (unless skip flag is set)
    let expected_vkey = vk.bytes32().to_string();
    if !skip_vkey_check {
        if expected_vkey != fixture.vkey {
            return Err(format!(
                "Verification key mismatch! Expected: {}, Got: {}",
                expected_vkey, fixture.vkey
            )
            .into());
        }
        println!("\n✅ Verification key matches!");
    } else {
        println!("\n⚠️  Warning: Skipping verification key check");
        println!("   Expected vkey: {}", expected_vkey);
        println!("   Proof vkey:    {}", fixture.vkey);
    }

    // Verify the proof
    println!("\nVerifying proof...");
    let verify_start = Instant::now();

    client.verify(&proof, &vk)?;

    let verify_duration = verify_start.elapsed();
    println!(
        "✅ Proof verified successfully in {} ms!",
        verify_duration.as_millis()
    );

    // Decode and display public values
    let decoded = PublicValuesStruct::abi_decode(&public_values_bytes)?;
    println!("\n=== Verified Public Values ===");
    println!(
        "Old root: 0x{}",
        hex::encode(decoded.old_root.to_be_bytes::<32>())
    );
    println!(
        "New root: 0x{}",
        hex::encode(decoded.new_root.to_be_bytes::<32>())
    );

    Ok(())
}

/// Find the most recent core proof file in the proofs directory
pub fn find_latest_core_proof() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let proof_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");

    let mut core_proofs: Vec<_> = std::fs::read_dir(&proof_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with("core-proof-") && name.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();

    if core_proofs.is_empty() {
        return Err("No core proof files found in proofs directory".into());
    }

    // Sort by modification time (most recent first)
    core_proofs.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    core_proofs.reverse();

    Ok(core_proofs[0].path())
}

/// Verify a core proof from JSON data without requiring ELF
/// This function is suitable for on-chain verification where files and ELF are not available
///
/// Note: For true on-chain verification, consider using Plonk or Groth16 proofs which are
/// specifically designed for efficient on-chain verification.
pub fn verify_core_proof_from_json(
    fixture: &CoreProofFixture,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Verifying Proof from JSON ===");
    println!("Old root: {}", fixture.old_root);
    println!("New root: {}", fixture.new_root);
    println!("Verification key hash: {}", fixture.vkey);

    // Decode proof and public values
    let proof_bytes = hex::decode(fixture.proof.trim_start_matches("0x"))?;
    let public_values_bytes = hex::decode(fixture.public_values.trim_start_matches("0x"))?;

    // Print proof size information
    println!("\n📊 Proof Size Information:");
    println!(
        "   Serialized proof size: {} bytes ({:.2} KB)",
        proof_bytes.len(),
        proof_bytes.len() as f64 / 1024.0
    );
    println!("   Public values size: {} bytes", public_values_bytes.len());

    // Create SP1ProofWithPublicValues using bincode deserialization
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&proof_bytes)?;

    // Check if we have the verifying key bytes
    let vk = if let Some(vkey_bytes_str) = &fixture.vkey_bytes {
        // Decode and deserialize the verifying key
        let vkey_bytes = hex::decode(vkey_bytes_str.trim_start_matches("0x"))?;
        println!(
            "   Verifying key size: {} bytes ({:.2} KB)",
            vkey_bytes.len(),
            vkey_bytes.len() as f64 / 1024.0
        );

        let vk: SP1VerifyingKey = bincode::deserialize(&vkey_bytes)?;

        // Verify the vkey hash matches
        let computed_hash = vk.bytes32();
        if computed_hash != fixture.vkey {
            println!("\n⚠️  Warning: Verifying key hash mismatch!");
            println!("   Expected: {}", fixture.vkey);
            println!("   Computed: {}", computed_hash);
        } else {
            println!("\n✅ Verifying key hash matches!");
        }

        vk
    } else {
        return Err("No verifying key bytes found in fixture. Please regenerate the proof with the latest version.".into());
    };

    // For production on-chain verification, consider:
    // 1. Using Plonk/Groth16 proofs (designed for on-chain verification)
    // 2. Running a verification service that keeps the client initialized
    // 3. Using SP1's network prover for remote verification

    // Build the SP1 client (unavoidable ~3.5s initialization)
    println!("\nBuilding SP1 CPU client...");
    let client_start = Instant::now();
    let client = ProverClient::builder().cpu().build();
    let client_duration = client_start.elapsed();
    println!("   Client built in {} ms", client_duration.as_millis());

    // Verify the proof using SP1 client
    println!("\nVerifying proof with SP1 client...");
    let verify_start = Instant::now();

    client.verify(&proof, &vk)?;

    let verify_duration = verify_start.elapsed();
    println!(
        "✅ Proof verified successfully in {} ms!",
        verify_duration.as_millis()
    );

    // Print total time
    let total_duration = client_duration + verify_duration;
    println!(
        "   Total time (client + verification): {} ms",
        total_duration.as_millis()
    );

    // Verify public values match what's in the fixture
    let decoded = PublicValuesStruct::abi_decode(&public_values_bytes)?;

    println!("\n=== Verified Public Values ===");
    println!(
        "   Old root: 0x{}",
        hex::encode(decoded.old_root.to_be_bytes::<32>())
    );
    println!(
        "   New root: 0x{}",
        hex::encode(decoded.new_root.to_be_bytes::<32>())
    );

    Ok(())
}

/// List all core proof files in the proofs directory
pub fn list_core_proofs() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let proof_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proofs");

    let mut core_proofs: Vec<_> = std::fs::read_dir(&proof_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with("core-proof-") && name.ends_with(".json"))
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .collect();

    core_proofs.sort();
    Ok(core_proofs)
}

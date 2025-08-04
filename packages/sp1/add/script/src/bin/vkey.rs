use sp1_sdk::{HashableKey, ProverClient, include_elf};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");

/// The ELF for the aggregation program.
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

fn main() {
    let client = ProverClient::from_env();
    
    // Setup and print add program vkey
    let (_, add_vk) = client.setup(ADD_ELF);
    let add_vkey_hash = add_vk.bytes32();
    println!("Add Program:");
    println!("  Verification Key Hash (32 bytes): {}", add_vkey_hash);
    
    // Setup and print aggregate program vkey
    let (_, aggregate_vk) = client.setup(AGGREGATE_ELF);
    let aggregate_vkey_hash = aggregate_vk.bytes32();
    println!("\nAggregate Program:");
    println!("  Verification Key Hash (32 bytes): {}", aggregate_vkey_hash);
    
    // Print Solana-specific instructions
    println!("\n  For Solana integration (when using aggregated proofs):");
    println!("  Update this value in solana/programs/add/src/lib.rs:");
    println!("  const ADD_VKEY_HASH: &str = \"{}\";", aggregate_vkey_hash);
}

use sp1_sdk::{HashableKey, ProverClient, include_elf};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");

/// The ELF for the aggregation program.
pub const AGGREGATE_ELF: &[u8] = include_elf!("aggregate-program");

fn main() {
    let client = ProverClient::from_env();
    
    // Setup and print add program vkey
    let (_, add_vk) = client.setup(ADD_ELF);
    println!("Add Program:");
    println!("  Verification Key Hash (32 bytes): {}", add_vk.bytes32());
    
    // Setup and print aggregate program vkey
    let (_, aggregate_vk) = client.setup(AGGREGATE_ELF);
    println!("\nAggregate Program:");
    println!("  Verification Key Hash (32 bytes): {}", aggregate_vk.bytes32());
}

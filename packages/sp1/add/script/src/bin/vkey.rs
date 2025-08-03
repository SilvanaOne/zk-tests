use sp1_sdk::{HashableKey, ProverClient, include_elf};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const ADD_ELF: &[u8] = include_elf!("add-program");

fn main() {
    let client = ProverClient::from_env();
    let (_, vk) = client.setup(ADD_ELF);

    // Print the verification key hash
    println!("Hash (32 bytes): {}", vk.bytes32());
}

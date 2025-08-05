#![no_main]
sp1_zkvm::entrypoint!(main);

use sha2::{Digest, Sha256};

pub fn main() {
    // Read the number of iterations
    let iterations = sp1_zkvm::io::read::<u32>();

    // Read the input data
    let input_u32 = sp1_zkvm::io::read::<Vec<u32>>();
    // Convert u32 array to bytes properly (little-endian)
    let input = input_u32.iter()
        .flat_map(|&x| x.to_le_bytes())
        .collect::<Vec<u8>>();

    // Verify that we have at least one proof
    assert!(iterations > 0, "Must have at least one iteration");
    let mut digest = Sha256::digest(&input);

    for _ in 1..iterations {
        digest = Sha256::digest(&input);
    }

    sp1_zkvm::io::commit_slice(&digest);
}

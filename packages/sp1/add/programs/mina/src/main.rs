#![no_main]
sp1_zkvm::entrypoint!(main);
mod poseidon;

use crypto_bigint::{U256, Encoding};
use poseidon::poseidon;

pub fn main() {
    let iterations = sp1_zkvm::io::read::<u32>();
    let input = sp1_zkvm::io::read::<Vec<u32>>();
    assert!(iterations > 0, "Must have at least one iteration");

    let input_u256: Vec<U256> = input.iter().map(|x| U256::from(*x as u64)).collect();

    // Perform hash calculations in the loop
    let mut digest = poseidon(input_u256.clone());
    for _ in 1..iterations {
        digest = poseidon(input_u256.clone());
    }

    let digest_bytes = digest.to_be_bytes();
    sp1_zkvm::io::commit_slice(&digest_bytes);
}

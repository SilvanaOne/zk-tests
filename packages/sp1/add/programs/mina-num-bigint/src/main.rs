#![no_main]
sp1_zkvm::entrypoint!(main);
mod poseidon;
use std::hash::Hash;

use num_bigint::BigInt;
use poseidon::poseidon;

pub fn main() {
    let iterations = sp1_zkvm::io::read::<u32>();
    let input = sp1_zkvm::io::read::<Vec<u32>>();
    assert!(iterations > 0, "Must have at least one iteration");

    let input_bigint: Vec<BigInt> = input.iter().map(|x| BigInt::from(*x)).collect();

    // Perform hash calculations in the loop
    let mut digest = poseidon(input_bigint.clone());
    for _ in 1..iterations {
        digest = poseidon(input_bigint.clone());
    }

    let (_, digest_bytes) = digest.to_bytes_be();
    sp1_zkvm::io::commit_slice(&digest_bytes);
}

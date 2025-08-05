#![no_main]
sp1_zkvm::entrypoint!(main);

use p3_baby_bear::BabyBear;
use p3_field::{AbstractField, PrimeField32};
use p3_symmetric::{CryptographicHasher, PaddingFreeSponge};
use sp1_primitives::poseidon2_init;

pub fn main() {
    let iterations = sp1_zkvm::io::read::<u32>();
    let input = sp1_zkvm::io::read::<Vec<u32>>();
    assert!(iterations > 0, "Must have at least one iteration");

    // Convert u32 input to BabyBear elements once at the beginning
    let elems: Vec<BabyBear> = input
        .iter()
        .map(|&x| BabyBear::from_canonical_u32(x))
        .collect();

    // Initialize Poseidon2 sponge once
    let perm = poseidon2_init();
    let sponge = PaddingFreeSponge::<_, 16, 8, 1>::new(perm);

    // Perform hash calculations in the loop
    let mut digest = sponge.hash_slice(&elems)[0];
    for _ in 1..iterations {
        digest = sponge.hash_slice(&elems)[0];
    }

    let out = digest.as_canonical_u32().to_le_bytes();
    sp1_zkvm::io::commit_slice(&out);
}

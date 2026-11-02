//! The Jellyfish tree, one witness per batch, with field-native Poseidon2.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Poseidon2, jellyfish::batch};

pub fn main() {
    merkle_guest::run(batch::run::<Poseidon2<Zk>>);
}

//! Stock Jellyfish update proofs with field-native Poseidon2 nodes.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{
    hash::Poseidon2,
    jellyfish::{hashers::*, stock},
};

pub fn main() {
    merkle_guest::run(stock::run::<Poseidon2<Zk>, PoseidonNode<Zk>>);
}

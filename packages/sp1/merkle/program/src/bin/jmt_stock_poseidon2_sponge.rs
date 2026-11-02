//! Stock Jellyfish update proofs with SP1's Poseidon2 byte sponge.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{
    hash::Poseidon2,
    jellyfish::{hashers::*, stock},
};

pub fn main() {
    merkle_guest::run(stock::run::<Poseidon2<Zk>, PoseidonSponge<Zk>>);
}

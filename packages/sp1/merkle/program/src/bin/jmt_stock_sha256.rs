//! Stock Jellyfish update proofs with raw SHA-256 behind the byte stream.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{
    hash::Sha256Raw,
    jellyfish::{hashers::*, stock},
};

pub fn main() {
    merkle_guest::run(stock::run::<Sha256Raw<Zk>, Sha256Stream<Zk>>);
}

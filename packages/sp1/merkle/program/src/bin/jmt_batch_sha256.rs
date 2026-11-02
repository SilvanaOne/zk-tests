//! The Jellyfish tree, one witness per batch, with raw SHA-256 calls.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Sha256Raw, jellyfish::batch};

pub fn main() {
    merkle_guest::run(batch::run::<Sha256Raw<Zk>>);
}

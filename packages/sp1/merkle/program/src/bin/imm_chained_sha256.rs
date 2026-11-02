//! The indexed map, one witness per record, with raw SHA-256 calls.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Sha256Raw, imm::chained};

pub fn main() {
    merkle_guest::run(chained::run::<Sha256Raw<Zk>>);
}

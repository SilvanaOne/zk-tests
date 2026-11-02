//! The indexed map, one witness per record, with field-native Poseidon2.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Poseidon2, imm::chained};

pub fn main() {
    merkle_guest::run(chained::run::<Poseidon2<Zk>>);
}

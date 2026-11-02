//! The indexed map, one witness per record, with the patched sha2 crate.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Sha2Crate, imm::chained};

pub fn main() {
    merkle_guest::run(chained::run::<Sha2Crate<Zk>>);
}

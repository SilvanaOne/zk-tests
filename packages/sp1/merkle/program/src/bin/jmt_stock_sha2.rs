//! Stock Jellyfish update proofs with the patched `sha2` crate, which is what `Sha256Jmt` uses.

#![no_main]
sp1_zkvm::entrypoint!(main);

use merkle_guest::Zk;
use merkle_map::{hash::Sha2Crate, jellyfish::stock};

pub fn main() {
    merkle_guest::run(stock::run::<Sha2Crate<Zk>, sha2::Sha256>);
}

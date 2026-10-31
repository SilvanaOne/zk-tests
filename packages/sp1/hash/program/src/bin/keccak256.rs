//! Keccak-256 of arbitrary bytes through the tiny-keccak crate, patched to use the Keccak precompile.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{DigestBuf, Input};
use tiny_keccak::{Hasher, Keccak};

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 4];
    let mut digest = DigestBuf::default();
    for set in input.sets() {
        let mut hasher = Keccak::v256();
        hasher.update(set);
        hasher.finalize(&mut digest.0);
        digest.xor_into(&mut acc);
    }
    input.commit(&acc);
}

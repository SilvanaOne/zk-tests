//! SHA-256 of arbitrary bytes through the sha2 crate, patched to use the SHA precompiles.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{DigestBuf, Input};
use sha2::{Digest, Sha256};

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 4];
    let mut digest = DigestBuf::default();
    for set in input.sets() {
        let mut hasher = Sha256::new();
        hasher.update(set);
        hasher.finalize_into((&mut digest.0).into());
        digest.xor_into(&mut acc);
    }
    input.commit(&acc);
}

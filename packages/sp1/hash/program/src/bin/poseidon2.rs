//! Poseidon2 of arbitrary bytes through SP1's length-prefixed byte sponge and the Poseidon2 precompile.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{Input, pack_words};
use sp1_zkvm::syscalls::Poseidon2ByteHash;

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 8];
    for set in input.sets() {
        let digest = Poseidon2ByteHash::hash(set);
        for (a, d) in acc.iter_mut().zip(digest) {
            *a ^= d as u64;
        }
    }
    input.commit(&pack_words(&acc));
}

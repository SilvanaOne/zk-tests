//! A Merkle node with one Poseidon2 precompile call and a feed-forward: `Trunc8(P(x) + x)`.
//!
//! This is the compression mode from the Poseidon2 paper (https://eprint.iacr.org/2023/323). It
//! costs 8 modular additions per node on top of `poseidon2_node`.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{
    Input, pack_words,
    poseidon2::{first_elements, permute_node},
};
use sp1_zkvm::syscalls::Poseidon2State;

/// The KoalaBear prime.
const P: u32 = 0x7f00_0001;

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 8];
    let mut state = Poseidon2State::default();
    for node in input.node_words() {
        permute_node(&mut state, node.as_ptr().cast());
        let output = first_elements(&mut state);
        for ((a, out), x) in acc.iter_mut().zip(output).zip(node) {
            // Both are canonical, so below 2^31, and the sum cannot overflow.
            let sum = out + x;
            *a ^= (if sum >= P { sum - P } else { sum }) as u64;
        }
    }
    input.commit(&pack_words(&acc));
}

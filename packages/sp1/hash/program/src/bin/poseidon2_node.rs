//! A Merkle node with one Poseidon2 precompile call: `Trunc8(P(left || right))`.
//!
//! This is the compression SP1 uses for its own Merkle commitments. It relies on every child being
//! a hash output, so leaves must be hashed.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{
    Input,
    poseidon2::{first_lanes, permute_node},
    xor,
};
use sp1_zkvm::syscalls::Poseidon2State;

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 4];
    let mut state = Poseidon2State::default();
    for node in input.nodes() {
        permute_node(&mut state, node);
        xor(&mut acc, &first_lanes(&mut state));
    }
    input.commit(&acc);
}

//! A Merkle node with one raw Keccak precompile call on native lanes.
//!
//! This is standard Keccak-256 of 64 bytes: the message is 8 little-endian lanes and the digest is
//! the first 4 lanes of the state.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{Input, xor};
use sp1_zkvm::syscalls::syscall_keccak_permute;

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 4];
    for node in input.nodes() {
        let mut state = [0u64; 25];
        state[..8].copy_from_slice(node);
        // Keccak-256 padding with a rate of 136 bytes: 0x01 right after the 64-byte message, and
        // 0x80 in the last byte of the rate.
        state[8] = 0x01;
        state[16] = 0x80 << 56;
        syscall_keccak_permute(&mut state);
        let lanes = [state[0], state[1], state[2], state[3]];
        xor(&mut acc, &lanes);
    }
    input.commit(&acc);
}

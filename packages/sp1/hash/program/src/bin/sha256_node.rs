//! A Merkle node with raw SHA precompile calls on native words.
//!
//! This is standard SHA-256 of 64 bytes: the 16 input words are the big-endian message words and
//! the digest is the 8 state words. A tree that holds its digests as words pays no byte swaps.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{Input, pack_words, xor};
use sp1_zkvm::syscalls::{syscall_sha256_compress, syscall_sha256_extend};

/// The SHA-256 initial state, one 32-bit word per `u64` as the syscalls expect.
const INITIAL_STATE: [u64; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

pub fn main() {
    let input = Input::read();

    // A 64-byte message always pads to the same second block, so its message schedule is computed
    // once and reused. SHA_COMPRESS only reads the schedule.
    let mut padding = [0u64; 64];
    padding[0] = 0x8000_0000;
    padding[15] = 512;
    syscall_sha256_extend(&mut padding);

    let mut acc = [0u64; 8];
    let mut schedule = [0u64; 64];
    for node in input.node_words() {
        for (slot, word) in schedule.iter_mut().zip(node) {
            *slot = *word as u64;
        }
        syscall_sha256_extend(&mut schedule);
        let mut state = INITIAL_STATE;
        syscall_sha256_compress(&mut schedule, &mut state);
        syscall_sha256_compress(&mut padding, &mut state);
        xor(&mut acc, &state);
    }
    input.commit(&pack_words(&acc));
}

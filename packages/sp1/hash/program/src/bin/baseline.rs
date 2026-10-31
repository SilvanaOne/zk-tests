//! No hash: reads every word of every set once, so subtracting this guest removes input handling.

#![no_main]
sp1_zkvm::entrypoint!(main);

use hash_guest::{Input, xor};

pub fn main() {
    let input = Input::read();
    let mut acc = [0u64; 4];
    let (words, words_per_set) = input.words();
    for set in words.chunks_exact(words_per_set) {
        let (lanes, rest) = set.as_chunks::<4>();
        for lane in lanes {
            xor(&mut acc, lane);
        }
        for (a, word) in acc.iter_mut().zip(rest) {
            *a ^= word;
        }
    }
    input.commit(&acc);
}

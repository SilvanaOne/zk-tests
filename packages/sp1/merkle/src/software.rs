//! The hash backends on the host: hardware SHA-256 and SP1's Poseidon2 permutation, counting the
//! precompile calls a guest would make for the same work.

use std::{
    cell::{Cell, UnsafeCell},
    sync::LazyLock,
};

use merkle_map::hash::{PermBackend, PermState, ShaBackend};
use serde::{Deserialize, Serialize};
use slop_algebra::{AbstractField, PrimeField32};
use slop_symmetric::Permutation;
use sp1_primitives::{SP1Field, SP1Perm, poseidon2_init};

/// Invocation counts of the hash precompiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyscallCounts {
    pub sha_extend: u64,
    pub sha_compress: u64,
    pub poseidon2: u64,
}

thread_local! {
    static COUNTS: Cell<SyscallCounts> = const { Cell::new(SyscallCounts { sha_extend: 0, sha_compress: 0, poseidon2: 0 }) };
    static SCHEDULE: UnsafeCell<[u64; 64]> = const { UnsafeCell::new([0; 64]) };
}

/// Returns the calls counted on this thread since the last call.
pub fn take_counts() -> SyscallCounts {
    COUNTS.take()
}

fn count(update: impl FnOnce(&mut SyscallCounts)) {
    let mut counts = COUNTS.get();
    update(&mut counts);
    COUNTS.set(counts);
}

static PERMUTATION: LazyLock<SP1Perm> = LazyLock::new(poseidon2_init);

/// The host backend.
pub struct Native;

impl ShaBackend for Native {
    fn schedule() -> *mut [u64; 64] {
        SCHEDULE.with(UnsafeCell::get)
    }

    unsafe fn extend(_w: *mut [u64; 64]) {
        // The host compresses from the message words alone.
        count(|counts| counts.sha_extend += 1);
    }

    unsafe fn compress(w: *const [u64; 64], state: &mut [u64; 8]) {
        let mut block = [0u8; 64];
        for (index, bytes) in block.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            // SAFETY: the caller initialized at least the first 16 words of the schedule.
            let word = unsafe { w.cast::<u64>().add(index).read() } as u32;
            *bytes = word.to_be_bytes();
        }
        let mut words: [u32; 8] = core::array::from_fn(|i| state[i] as u32);
        sha2::compress256(&mut words, &[block.into()]);
        *state = words.map(u64::from);
        count(|counts| counts.sha_compress += 1);
    }

    fn note_crate_blocks(blocks: u64) {
        count(|counts| {
            counts.sha_extend += blocks;
            counts.sha_compress += blocks;
        });
    }
}

impl PermBackend for Native {
    unsafe fn permute(state: *mut PermState) {
        // SAFETY: the caller provides 16 initialized elements.
        let state = unsafe { &mut *state };
        let mut elements = state.0.map(SP1Field::from_canonical_u32);
        PERMUTATION.permute_mut(&mut elements);
        state.0 = elements.map(|element| element.as_canonical_u32());
        count(|counts| counts.poseidon2 += 1);
    }
}

#[cfg(test)]
mod tests {
    use merkle_map::hash::{ImmHasher, POSEIDON_EMPTY, Poseidon2, tag};

    use super::*;

    /// `EMPTY` is the hash of a tag, so it cannot be a node or leaf hash without a collision.
    #[test]
    fn poseidon_empty_is_the_hash_of_its_tag() {
        let mut state = PermState([0; 16]);
        state.0[15] = tag::EMPTY;
        // SAFETY: the state is 16 canonical elements.
        unsafe { Native::permute(&mut state) };
        let expected: [u32; 8] = core::array::from_fn(|i| state.0[i]);
        assert_eq!(
            POSEIDON_EMPTY.0, expected,
            "set POSEIDON_EMPTY to {expected:?}"
        );
        assert_eq!(Poseidon2::<Native>::EMPTY.0, expected);
    }
}

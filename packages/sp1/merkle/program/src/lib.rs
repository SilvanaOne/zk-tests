//! The guest side of the benchmark: the precompiles as a hash backend, and the input and output
//! around each engine.

use merkle_map::{
    hash::{PermBackend, PermState, ShaBackend},
    record::PublicValues,
};
use sp1_zkvm::{
    io,
    syscalls::{Poseidon2State, syscall_poseidon2, syscall_sha256_compress, syscall_sha256_extend},
};

/// The SP1 precompiles.
pub struct Zk;

/// A static is in the program's image, so the prover includes it however it is used.
static mut SCHEDULE: [u64; 64] = [0; 64];

impl ShaBackend for Zk {
    #[inline(always)]
    fn schedule() -> *mut [u64; 64] {
        &raw mut SCHEDULE
    }

    #[inline(always)]
    unsafe fn extend(w: *mut [u64; 64]) {
        syscall_sha256_extend(w);
    }

    #[inline(always)]
    unsafe fn compress(w: *const [u64; 64], state: &mut [u64; 8]) {
        // The precompile only reads the schedule, so it is safe to pass a shared one.
        syscall_sha256_compress(w.cast_mut(), state);
    }
}

impl PermBackend for Zk {
    #[inline(always)]
    unsafe fn permute(state: *mut PermState) {
        // SAFETY: both types are 16 `u32`s, `repr(C)` and 8-aligned, and the caller provides a valid
        // state.
        unsafe { syscall_poseidon2(&mut *state.cast::<Poseidon2State>()) };
    }
}

/// Reads the witness, runs an engine and commits its public values.
#[inline(always)]
pub fn run(engine: fn(&[u8]) -> PublicValues) {
    let witness = io::read_vec();
    let output = engine(&witness);
    io::commit_slice(output.as_bytes());
}

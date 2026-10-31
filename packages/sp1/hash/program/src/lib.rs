//! Input and output handling shared by the benchmark guests.
//!
//! Every guest reads the same input format and commits the same 48 bytes, so the differences
//! between guests come from the hash path. Committing more than this would add SHA-256 work to
//! every guest, because SP1 hashes all public values with SHA-256 inside the guest.

use sp1_zkvm::io;

/// The digest accumulator, XORed over every hash and committed as public output.
pub type Lanes = [u64; 4];

/// The sets to hash. Each one starts on an 8-byte boundary at a multiple of the stride.
pub struct Input {
    size: usize,
    count: usize,
    stride: usize,
    data: Vec<u8>,
}

impl Input {
    /// Reads the set size, the set count and the flat data buffer.
    ///
    /// `read_vec` takes the buffer as it is, with no per-byte deserialization in the guest.
    pub fn read() -> Self {
        let size = io::read::<u64>() as usize;
        let count = io::read::<u64>() as usize;
        let data = io::read_vec();
        let stride = size.next_multiple_of(8);
        assert_eq!(data.len(), count * stride);
        assert!(data.as_ptr().addr().is_multiple_of(8));
        Self {
            size,
            count,
            stride,
            data,
        }
    }

    /// Yields the bytes of each set.
    pub fn sets(&self) -> impl Iterator<Item = &[u8]> {
        self.data
            .chunks_exact(self.stride)
            .map(|set| &set[..self.size])
    }

    /// Returns all sets as one run of 64-bit words, with the number of words per set.
    pub fn words(&self) -> (&[u64], usize) {
        // SAFETY: `read` checked the alignment, and the length is a multiple of 8 by construction.
        let words =
            unsafe { core::slice::from_raw_parts(self.data.as_ptr().cast(), self.data.len() / 8) };
        (words, self.stride / 8)
    }

    /// Returns each 64-byte set as 8 lanes.
    pub fn nodes(&self) -> &[[u64; 8]] {
        assert_eq!(self.size, 64);
        // SAFETY: `read` checked the alignment and that the buffer holds `count` sets of 64 bytes.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr().cast(), self.count) }
    }

    /// Returns each 64-byte set as 16 words.
    pub fn node_words(&self) -> &[[u32; 16]] {
        assert_eq!(self.size, 64);
        // SAFETY: `read` checked the alignment and that the buffer holds `count` sets of 64 bytes.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr().cast(), self.count) }
    }

    /// Commits `count || size || acc`, little-endian, 48 bytes in every guest.
    pub fn commit(&self, acc: &Lanes) {
        let output = [
            self.count as u64,
            self.size as u64,
            acc[0],
            acc[1],
            acc[2],
            acc[3],
        ];
        // SAFETY: any `[u64; 6]` is valid to read as 48 bytes.
        let bytes = unsafe { core::slice::from_raw_parts(output.as_ptr().cast::<u8>(), 48) };
        io::commit_slice(bytes);
    }
}

/// A digest buffer that can be read back as lanes with plain 64-bit loads.
#[repr(align(8))]
#[derive(Default)]
pub struct DigestBuf(pub [u8; 32]);

impl DigestBuf {
    #[inline(always)]
    pub fn xor_into(&self, acc: &mut Lanes) {
        // SAFETY: the buffer is 8-aligned and 32 bytes long, and any bytes are valid `u64`s.
        let lanes = unsafe { &*self.0.as_ptr().cast::<Lanes>() };
        xor(acc, lanes);
    }
}

#[inline(always)]
pub fn xor<const N: usize>(acc: &mut [u64; N], digest: &[u64; N]) {
    for (a, d) in acc.iter_mut().zip(digest) {
        *a ^= d;
    }
}

/// Packs 8 accumulated 32-bit words into lanes, once, after the hashing loop.
pub fn pack_words(words: &[u64; 8]) -> Lanes {
    core::array::from_fn(|i| words[2 * i] | words[2 * i + 1] << 32)
}

/// The Poseidon2 state and the node written into it, for the two node guests.
///
/// The syscall takes canonical `u32` elements, laid out in memory as 8 little-endian lanes.
pub mod poseidon2 {
    use sp1_zkvm::syscalls::Poseidon2State;

    /// Replaces the whole state with a node and permutes it.
    #[inline(always)]
    pub fn permute_node(state: &mut Poseidon2State, node: *const [u64; 8]) {
        // SAFETY: the state is `repr(align(8))` and holds 16 `u32`s, the size of `[u64; 8]`, and
        // the guests pass a pointer into a live slice of nodes.
        unsafe { state.as_mut_ptr().cast::<[u64; 8]>().write(node.read()) };
        state.permute();
    }

    /// Reads the first 4 lanes of the state, which are its first 8 elements.
    #[inline(always)]
    pub fn first_lanes(state: &mut Poseidon2State) -> [u64; 4] {
        // SAFETY: the state is 8-aligned and longer than 4 lanes, and any bytes are valid `u64`s.
        unsafe { state.as_mut_ptr().cast::<[u64; 4]>().read() }
    }

    /// Reads the first 8 elements of the state.
    #[inline(always)]
    pub fn first_elements(state: &mut Poseidon2State) -> [u32; 8] {
        // SAFETY: the state holds 16 `u32`s, and any bytes are valid `u32`s.
        unsafe { state.as_mut_ptr().cast::<[u32; 8]>().read() }
    }
}

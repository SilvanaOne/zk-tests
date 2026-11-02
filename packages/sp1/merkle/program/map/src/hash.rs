//! Digests, the traits the maps are generic over, and the hash profiles.
//!
//! The traits are at the level of tree operations rather than a byte stream, because the cheap paths
//! are not byte streams: a raw SHA-256 node reuses one message schedule, and a Poseidon2 node is one
//! permutation of 16 field elements. Each profile runs on a backend: the precompiles in a guest, and
//! software that counts the same calls on the host.

use core::{marker::PhantomData, mem::MaybeUninit};

use sha2::Digest as _;

use crate::record::Record;

/// A 32-byte digest in the hash's native words, aligned so it moves with 64-bit loads and stores.
///
/// SHA-256 holds big-endian words, so word order is byte order. Poseidon2 holds 8 canonical KoalaBear
/// elements.
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Digest(pub [u32; 8]);

impl Digest {
    /// The smallest key, and the end-of-list marker in the indexed map.
    pub const ZERO: Self = Self([0; 8]);
}

impl borsh::BorshSerialize for Digest {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        // SAFETY: a digest is 32 bytes with no padding.
        let bytes = unsafe { &*(self as *const Self).cast::<[u8; 32]>() };
        writer.write_all(bytes)
    }
}

impl borsh::BorshDeserialize for Digest {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut digest = MaybeUninit::<Self>::uninit();
        // SAFETY: the buffer is 32 bytes, zeroed or not, and any 32 bytes are a valid digest.
        unsafe {
            let bytes = &mut *digest.as_mut_ptr().cast::<MaybeUninit<[u8; 32]>>();
            let bytes = bytes.write([0; 32]);
            reader.read_exact(bytes)?;
            Ok(digest.assume_init())
        }
    }
}

/// What every map needs from a hash.
pub trait Hasher {
    /// Goes into the public values, so a verifier can tell which hash a root belongs to.
    const ID: u8;
    /// The hash of arbitrary bytes.
    fn bytes(data: &[u8]) -> Digest;
    /// One step of the batch commitment, which binds the operation and order of every record.
    fn chain(prev: &Digest, tag: u32, key: &Digest, arg: &Digest) -> Digest;
    /// The batch commitment step a verifier outside the proof can recompute from the record alone.
    /// The same as `chain` unless the hash has no external verifier.
    fn chain_external(
        prev: &Digest,
        tag: u32,
        key: &Digest,
        arg: &Digest,
        _record: &Record,
    ) -> Digest {
        Self::chain(prev, tag, key, arg)
    }
    /// The canonical bytes of a digest.
    fn export(digest: &Digest) -> [u8; 32];
    /// The bytes of a commitment built with `chain_external`.
    fn export_external(digest: &Digest) -> [u8; 32] {
        Self::export(digest)
    }
    /// The inverse of `export`. Does not check that the bytes are canonical.
    fn import(bytes: &[u8; 32]) -> Digest;
}

/// The indexed map's tree operations.
pub trait ImmHasher: Hasher {
    /// An absent node. Two absent children give an absent parent, without hashing.
    const EMPTY: Digest;
    fn node(left: &Digest, right: &Digest) -> Digest;
    fn leaf(key: &Digest, value: &Digest, next_key: &Digest) -> Digest;
    /// Binds the number of leaves into the root.
    fn bind(internal_root: &Digest, length: u64) -> Digest;
    /// A key from a sequence number, for append-only sets whose keys are not hashed.
    fn encode_sequence(sequence: u64) -> Digest;

    /// `node`, except that two absent children stay absent.
    #[inline(always)]
    fn node_or_empty(left: &Digest, right: &Digest) -> Digest {
        if *left == Self::EMPTY && *right == Self::EMPTY {
            Self::EMPTY
        } else {
            Self::node(left, right)
        }
    }
}

/// The Jellyfish tree's operations, which must give the same roots as `jmt` with the same hash.
pub trait JmtHasher: Hasher {
    /// An empty subtree.
    const PLACEHOLDER: Digest;
    fn internal(left: &Digest, right: &Digest) -> Digest;
    fn leaf(key_hash: &Digest, value_hash: &Digest) -> Digest;
    /// Words whose numeric order and bits, most significant first, are the tree's path order.
    fn path_key(digest: &Digest) -> Digest;
}

/// The SHA-256 calls, on the raw words the precompiles use: one 32-bit word per `u64`.
pub trait ShaBackend {
    /// The one buffer every message schedule is built in.
    ///
    /// SP1's prover only includes memory that a CPU load or store touched, or that is in the
    /// program's image. `extend` is the only writer of words 16 to 63, so a schedule on stack the CPU
    /// never touched executes normally and then gives a proof that fails verification. Zeroing a
    /// schedule for every block also works, but costs 48 more stores per block.
    fn schedule() -> *mut [u64; 64];
    /// Fills in words 16 to 63 of the message schedule.
    ///
    /// # Safety
    /// `w` must point to 64 writable words, with the first 16 initialized.
    unsafe fn extend(w: *mut [u64; 64]);
    /// Runs one compression over a complete schedule.
    ///
    /// # Safety
    /// `w` must point to a schedule filled in by `extend`, or computed the same way.
    unsafe fn compress(w: *const [u64; 64], state: &mut [u64; 8]);
    /// Records blocks hashed through the `sha2` crate, which a guest's patch turns into precompile
    /// calls. A guest has nothing to record.
    fn note_crate_blocks(_blocks: u64) {}
}

/// The Poseidon2 state, laid out as the precompile expects it.
#[repr(C, align(8))]
pub struct PermState(pub [u32; 16]);

/// The Poseidon2 permutation.
pub trait PermBackend {
    /// # Safety
    /// `state` must point to 16 initialized, canonical elements.
    unsafe fn permute(state: *mut PermState);
}

/// The SHA-256 initial state, one 32-bit word per `u64` as the precompile expects.
const SHA_INITIAL: [u64; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The message schedule of the padding block of any 64-byte message. Computed at build time and
/// shared by every node, since compression only reads it.
static SHA_PADDING_64: [u64; 64] = padding_schedule_64();

const fn padding_schedule_64() -> [u64; 64] {
    let mut w = [0u32; 64];
    w[0] = 0x8000_0000;
    w[15] = 512;
    let mut i = 16;
    while i < 64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
        i += 1;
    }
    let mut out = [0u64; 64];
    let mut i = 0;
    while i < 64 {
        out[i] = w[i] as u64;
        i += 1;
    }
    out
}

/// Writes one word of a message schedule.
#[inline(always)]
fn set(w: *mut u64, index: usize, word: u32) {
    debug_assert!(index < 16);
    // SAFETY: every caller is a `sha_block` fill closure, whose `w` holds 64 words.
    unsafe { w.add(index).write(word as u64) }
}

/// Extends and compresses one block. `fill` writes the first 16 words of the schedule.
#[inline(always)]
fn sha_block<B: ShaBackend>(state: &mut [u64; 8], fill: impl FnOnce(*mut u64)) {
    let w = B::schedule();
    fill(w.cast());
    // SAFETY: `fill` wrote the first 16 words, and the schedule holds 64.
    unsafe {
        B::extend(w);
        B::compress(w, state);
    }
}

#[inline(always)]
fn sha_digest(state: &[u64; 8]) -> Digest {
    Digest(core::array::from_fn(|i| state[i] as u32))
}

fn sha_export(digest: &Digest) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(&digest.0) {
        *chunk = word.to_be_bytes();
    }
    bytes
}

fn sha_import(bytes: &[u8; 32]) -> Digest {
    let words = bytes.as_chunks::<4>().0;
    Digest(core::array::from_fn(|i| u32::from_be_bytes(words[i])))
}

/// Standard SHA-256 of arbitrary bytes through the raw calls.
fn sha_bytes<B: ShaBackend>(data: &[u8]) -> [u64; 8] {
    let mut state = SHA_INITIAL;
    let (blocks, rest) = data.as_chunks::<64>();
    for block in blocks {
        sha_load_block::<B>(&mut state, block);
    }
    let mut last = [0u8; 64];
    last[..rest.len()].copy_from_slice(rest);
    last[rest.len()] = 0x80;
    if rest.len() >= 56 {
        sha_load_block::<B>(&mut state, &last);
        last = [0u8; 64];
    }
    last[56..].copy_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    sha_load_block::<B>(&mut state, &last);
    state
}

#[inline(always)]
fn sha_load_block<B: ShaBackend>(state: &mut [u64; 8], block: &[u8; 64]) {
    sha_block::<B>(state, |w| {
        for (index, word) in block.as_chunks::<4>().0.iter().enumerate() {
            set(w, index, u32::from_be_bytes(*word));
        }
    });
}

/// The batch commitment over 100 bytes: the tag, then three digests. Two blocks.
#[inline(always)]
fn sha_chain<B: ShaBackend>(prev: &Digest, tag: u32, key: &Digest, arg: &Digest) -> Digest {
    let mut state = SHA_INITIAL;
    sha_block::<B>(&mut state, |w| {
        set(w, 0, tag);
        for i in 0..8 {
            set(w, 1 + i, prev.0[i]);
        }
        for i in 0..7 {
            set(w, 9 + i, key.0[i]);
        }
    });
    sha_block::<B>(&mut state, |w| {
        set(w, 0, key.0[7]);
        for i in 0..7 {
            set(w, 1 + i, arg.0[i]);
        }
        set(w, 8, arg.0[7]);
        set(w, 9, 0x8000_0000);
        for i in 10..15 {
            set(w, i, 0);
        }
        set(w, 15, 800);
    });
    sha_digest(&state)
}

/// SHA-256 through the raw precompile calls, on native words.
pub struct Sha256Raw<B>(PhantomData<B>);

impl<B: ShaBackend> Hasher for Sha256Raw<B> {
    const ID: u8 = 2;

    fn bytes(data: &[u8]) -> Digest {
        sha_digest(&sha_bytes::<B>(data))
    }

    #[inline(always)]
    fn chain(prev: &Digest, tag: u32, key: &Digest, arg: &Digest) -> Digest {
        sha_chain::<B>(prev, tag, key, arg)
    }

    fn export(digest: &Digest) -> [u8; 32] {
        sha_export(digest)
    }

    fn import(bytes: &[u8; 32]) -> Digest {
        sha_import(bytes)
    }
}

impl<B: ShaBackend> ImmHasher for Sha256Raw<B> {
    const EMPTY: Digest = Digest::ZERO;

    /// 64 bytes: one block of message, and one padding block with a shared schedule.
    #[inline(always)]
    fn node(left: &Digest, right: &Digest) -> Digest {
        let mut state = SHA_INITIAL;
        sha_block::<B>(&mut state, |w| {
            for i in 0..8 {
                set(w, i, left.0[i]);
                set(w, 8 + i, right.0[i]);
            }
        });
        // SAFETY: the shared schedule is complete, and compression only reads it.
        unsafe { B::compress(&raw const SHA_PADDING_64, &mut state) };
        sha_digest(&state)
    }

    /// 96 bytes, two blocks.
    #[inline(always)]
    fn leaf(key: &Digest, value: &Digest, next_key: &Digest) -> Digest {
        let mut state = SHA_INITIAL;
        sha_block::<B>(&mut state, |w| {
            for i in 0..8 {
                set(w, i, key.0[i]);
                set(w, 8 + i, value.0[i]);
            }
        });
        sha_block::<B>(&mut state, |w| {
            for i in 0..8 {
                set(w, i, next_key.0[i]);
            }
            set(w, 8, 0x8000_0000);
            for i in 9..15 {
                set(w, i, 0);
            }
            set(w, 15, 768);
        });
        sha_digest(&state)
    }

    /// 40 bytes, one block: the root, then the length as a big-endian `u64`.
    #[inline(always)]
    fn bind(internal_root: &Digest, length: u64) -> Digest {
        let mut state = SHA_INITIAL;
        sha_block::<B>(&mut state, |w| {
            for i in 0..8 {
                set(w, i, internal_root.0[i]);
            }
            set(w, 8, (length >> 32) as u32);
            set(w, 9, length as u32);
            set(w, 10, 0x8000_0000);
            for i in 11..15 {
                set(w, i, 0);
            }
            set(w, 15, 320);
        });
        sha_digest(&state)
    }

    #[inline(always)]
    fn encode_sequence(sequence: u64) -> Digest {
        let mut words = [0u32; 8];
        words[6] = (sequence >> 32) as u32;
        words[7] = sequence as u32;
        Digest(words)
    }
}

/// `b"JMT::IntrnalNode"` as big-endian words.
const JMT_INTERNAL_WORDS: [u32; 4] = [0x4a4d_543a, 0x3a49_6e74, 0x726e_616c, 0x4e6f_6465];
/// The first 12 bytes of `b"JMT::LeafNode"` as big-endian words, and its last byte.
const JMT_LEAF_WORDS: [u32; 3] = [0x4a4d_543a, 0x3a4c_6561, 0x664e_6f64];
const JMT_LEAF_LAST_BYTE: u32 = b'e' as u32;

/// `jmt`'s empty-subtree constant, read as `import` reads a digest.
const JMT_PLACEHOLDER_BYTES: [u8; 32] = *b"SPARSE_MERKLE_PLACEHOLDER_HASH__";

const fn words_be(bytes: &[u8; 32]) -> [u32; 8] {
    let mut words = [0u32; 8];
    let mut i = 0;
    while i < 8 {
        words[i] = u32::from_be_bytes([
            bytes[4 * i],
            bytes[4 * i + 1],
            bytes[4 * i + 2],
            bytes[4 * i + 3],
        ]);
        i += 1;
    }
    words
}

const fn words_le(bytes: &[u8; 32]) -> [u32; 8] {
    let mut words = [0u32; 8];
    let mut i = 0;
    while i < 8 {
        words[i] = u32::from_le_bytes([
            bytes[4 * i],
            bytes[4 * i + 1],
            bytes[4 * i + 2],
            bytes[4 * i + 3],
        ]);
        i += 1;
    }
    words
}

impl<B: ShaBackend> JmtHasher for Sha256Raw<B> {
    const PLACEHOLDER: Digest = Digest(words_be(&JMT_PLACEHOLDER_BYTES));

    /// 80 bytes, two blocks: the 16-byte separator pushes the right child into the second.
    #[inline(always)]
    fn internal(left: &Digest, right: &Digest) -> Digest {
        let mut state = SHA_INITIAL;
        sha_block::<B>(&mut state, |w| {
            for (i, word) in JMT_INTERNAL_WORDS.iter().enumerate() {
                set(w, i, *word);
            }
            for i in 0..8 {
                set(w, 4 + i, left.0[i]);
            }
            for i in 0..4 {
                set(w, 12 + i, right.0[i]);
            }
        });
        sha_block::<B>(&mut state, |w| {
            for i in 0..4 {
                set(w, i, right.0[4 + i]);
            }
            set(w, 4, 0x8000_0000);
            for i in 5..15 {
                set(w, i, 0);
            }
            set(w, 15, 640);
        });
        sha_digest(&state)
    }

    /// 77 bytes, two blocks: the 13-byte separator shifts every child word by one byte.
    #[inline(always)]
    fn leaf(key_hash: &Digest, value_hash: &Digest) -> Digest {
        let mut state = SHA_INITIAL;
        let (k, v) = (&key_hash.0, &value_hash.0);
        sha_block::<B>(&mut state, |w| {
            for (i, word) in JMT_LEAF_WORDS.iter().enumerate() {
                set(w, i, *word);
            }
            set(w, 3, (JMT_LEAF_LAST_BYTE << 24) | (k[0] >> 8));
            for i in 1..8 {
                set(w, 3 + i, (k[i - 1] << 24) | (k[i] >> 8));
            }
            set(w, 11, (k[7] << 24) | (v[0] >> 8));
            for i in 1..5 {
                set(w, 11 + i, (v[i - 1] << 24) | (v[i] >> 8));
            }
        });
        sha_block::<B>(&mut state, |w| {
            for i in 5..8 {
                set(w, i - 5, (v[i - 1] << 24) | (v[i] >> 8));
            }
            set(w, 3, (v[7] << 24) | 0x0080_0000);
            for i in 4..15 {
                set(w, i, 0);
            }
            set(w, 15, 616);
        });
        sha_digest(&state)
    }

    #[inline(always)]
    fn path_key(digest: &Digest) -> Digest {
        *digest
    }
}

/// SHA-256 through the `sha2` crate, whose patch calls the precompiles. It gives the same digests as
/// `Sha256Raw`, and shows what the crate path costs on real tree operations.
pub struct Sha2Crate<B>(PhantomData<B>);

impl<B: ShaBackend> Sha2Crate<B> {
    fn hash(parts: &[&[u8]]) -> Digest {
        let mut hasher = sha2::Sha256::new();
        let mut length = 0u64;
        for part in parts {
            hasher.update(part);
            length += part.len() as u64;
        }
        B::note_crate_blocks((length + 8) / 64 + 1);
        sha_import(&hasher.finalize().into())
    }
}

impl<B: ShaBackend> Hasher for Sha2Crate<B> {
    const ID: u8 = 1;

    fn bytes(data: &[u8]) -> Digest {
        Self::hash(&[data])
    }

    fn chain(prev: &Digest, tag: u32, key: &Digest, arg: &Digest) -> Digest {
        Self::hash(&[
            &tag.to_be_bytes(),
            &sha_export(prev),
            &sha_export(key),
            &sha_export(arg),
        ])
    }

    fn export(digest: &Digest) -> [u8; 32] {
        sha_export(digest)
    }

    fn import(bytes: &[u8; 32]) -> Digest {
        sha_import(bytes)
    }
}

impl<B: ShaBackend> ImmHasher for Sha2Crate<B> {
    const EMPTY: Digest = Digest::ZERO;

    fn node(left: &Digest, right: &Digest) -> Digest {
        Self::hash(&[&sha_export(left), &sha_export(right)])
    }

    fn leaf(key: &Digest, value: &Digest, next_key: &Digest) -> Digest {
        Self::hash(&[&sha_export(key), &sha_export(value), &sha_export(next_key)])
    }

    fn bind(internal_root: &Digest, length: u64) -> Digest {
        Self::hash(&[&sha_export(internal_root), &length.to_be_bytes()])
    }

    fn encode_sequence(sequence: u64) -> Digest {
        Sha256Raw::<B>::encode_sequence(sequence)
    }
}

/// Domain tags, placed in the sponge's capacity so no two uses share an input shape.
pub mod tag {
    pub const IMM_LEAF: u32 = 1;
    pub const IMM_BIND: u32 = 2;
    pub const JMT_LEAF: u32 = 3;
    pub const CHAIN: u32 = 0x100;
    pub const EMPTY: u32 = 5;
}

/// The bytes one absorb takes: 8 rate elements of 3 bytes each.
const POSEIDON_BYTE_BLOCK: usize = 24;

/// Poseidon2 over KoalaBear with field-native tree operations.
///
/// A node is `Trunc8(P(left || right))`, the form SP1 uses for its own Merkle commitments. That form
/// is only safe if every leaf is a proper hash output and empty nodes are not zero, so leaves are
/// tagged sponges and `EMPTY` is the hash of a tag.
pub struct Poseidon2<B>(PhantomData<B>);

impl<B: PermBackend> Poseidon2<B> {
    /// `Trunc8(P(left || right))`.
    #[inline(always)]
    pub fn compress(left: &Digest, right: &Digest) -> Digest {
        let mut state = MaybeUninit::<PermState>::uninit();
        let half = state.as_mut_ptr().cast::<Digest>();
        // SAFETY: the state is 8-aligned and holds exactly two digests, all written before the
        // permutation.
        unsafe {
            half.write(*left);
            half.add(1).write(*right);
            B::permute(state.as_mut_ptr());
            half.read()
        }
    }

    /// A sponge over whole digests, with the tag in the capacity. Gives both halves of the last
    /// state.
    #[inline(always)]
    fn sponge_state(capacity: &Digest, blocks: &[&Digest]) -> (Digest, Digest) {
        let mut state = MaybeUninit::<PermState>::uninit();
        let half = state.as_mut_ptr().cast::<Digest>();
        // SAFETY: as in `compress`. The capacity half is written once and then kept, and each block
        // overwrites the rate half.
        unsafe {
            half.add(1).write(*capacity);
            for block in blocks {
                half.write(**block);
                B::permute(state.as_mut_ptr());
            }
            (half.read(), half.add(1).read())
        }
    }

    #[inline(always)]
    fn sponge(capacity: &Digest, blocks: &[&Digest]) -> Digest {
        Self::sponge_state(capacity, blocks).0
    }

    #[inline(always)]
    fn capacity(tag: u32) -> Digest {
        let mut words = [0u32; 8];
        words[7] = tag;
        Digest(words)
    }

    /// SP1's `Poseidon2ByteHash`, on this backend: a length block, then the data in zero-padded
    /// 24-byte blocks, overwriting the rate before each permutation.
    pub fn byte_sponge(data: &[u8]) -> Digest {
        let mut state = PermState([0; 16]);
        let mut length_block = [0u8; POSEIDON_BYTE_BLOCK];
        length_block[..8].copy_from_slice(&(data.len() as u64).to_le_bytes());
        Self::absorb_bytes(&mut state, &length_block);

        let (blocks, rest) = data.as_chunks::<POSEIDON_BYTE_BLOCK>();
        for block in blocks {
            Self::absorb_bytes(&mut state, block);
        }
        if !rest.is_empty() {
            let mut last = [0u8; POSEIDON_BYTE_BLOCK];
            last[..rest.len()].copy_from_slice(rest);
            Self::absorb_bytes(&mut state, &last);
        }
        Digest(core::array::from_fn(|i| state.0[i]))
    }

    #[inline(always)]
    fn absorb_bytes(state: &mut PermState, block: &[u8; POSEIDON_BYTE_BLOCK]) {
        for (element, bytes) in state.0.iter_mut().zip(block.as_chunks::<3>().0) {
            *element = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
        }
        // SAFETY: the state is initialized, and 3-byte elements are below the modulus.
        unsafe { B::permute(state) };
    }
}

impl<B: PermBackend + ShaBackend> Hasher for Poseidon2<B> {
    const ID: u8 = 3;

    fn bytes(data: &[u8]) -> Digest {
        Self::byte_sponge(data)
    }

    #[inline(always)]
    fn chain(prev: &Digest, tag: u32, key: &Digest, arg: &Digest) -> Digest {
        Self::sponge(&Self::capacity(tag::CHAIN + tag), &[prev, key, arg])
    }

    /// Nothing outside the proof can compute Poseidon2, so an external commitment to the batch uses
    /// SHA-256 throughout, over the record's own bytes: the same chain as the SHA-256 profiles.
    #[inline(always)]
    fn chain_external(
        prev: &Digest,
        tag: u32,
        _key: &Digest,
        _arg: &Digest,
        record: &Record,
    ) -> Digest {
        let key = sha_digest(&sha_bytes::<B>(record.key));
        let arg = sha_digest(&sha_bytes::<B>(record.arg));
        sha_chain::<B>(prev, tag, &key, &arg)
    }

    fn export_external(digest: &Digest) -> [u8; 32] {
        sha_export(digest)
    }

    fn export(digest: &Digest) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(&digest.0) {
            *chunk = word.to_le_bytes();
        }
        bytes
    }

    fn import(bytes: &[u8; 32]) -> Digest {
        let words = bytes.as_chunks::<4>().0;
        Digest(core::array::from_fn(|i| u32::from_le_bytes(words[i])))
    }
}

impl<B: PermBackend + ShaBackend> ImmHasher for Poseidon2<B> {
    /// `Trunc8(P(tag))`: `tag::EMPTY` in the last element and zeros elsewhere.
    const EMPTY: Digest = POSEIDON_EMPTY;

    #[inline(always)]
    fn node(left: &Digest, right: &Digest) -> Digest {
        Self::compress(left, right)
    }

    /// Three permutations over 24 canonical elements, with a leaf tag in the capacity.
    #[inline(always)]
    fn leaf(key: &Digest, value: &Digest, next_key: &Digest) -> Digest {
        Self::sponge(&Self::capacity(tag::IMM_LEAF), &[key, value, next_key])
    }

    /// One permutation, with the length as three 24-bit elements, which stay below the modulus.
    #[inline(always)]
    fn bind(internal_root: &Digest, length: u64) -> Digest {
        let mut capacity = Self::capacity(tag::IMM_BIND);
        capacity.0[0] = (length & 0xff_ffff) as u32;
        capacity.0[1] = ((length >> 24) & 0xff_ffff) as u32;
        capacity.0[2] = (length >> 48) as u32;
        Self::sponge(&capacity, &[internal_root])
    }

    /// Most significant element first, so element order is numeric order.
    #[inline(always)]
    fn encode_sequence(sequence: u64) -> Digest {
        let mut words = [0u32; 8];
        words[5] = (sequence >> 48) as u32;
        words[6] = ((sequence >> 24) & 0xff_ffff) as u32;
        words[7] = (sequence & 0xff_ffff) as u32;
        Digest(words)
    }
}

impl<B: PermBackend + ShaBackend> JmtHasher for Poseidon2<B> {
    /// The placeholder's top bytes are ASCII, so each little-endian word is a canonical element.
    const PLACEHOLDER: Digest = Digest(words_le(&JMT_PLACEHOLDER_BYTES));

    #[inline(always)]
    fn internal(left: &Digest, right: &Digest) -> Digest {
        Self::compress(left, right)
    }

    /// Two permutations, with a leaf tag in the capacity. The digest is the second half of the last
    /// state, because the first half would be a node: `internal(value_hash, capacity)`.
    ///
    /// This tree has no fixed depth, so a witness could then open a leaf as a branch, and its value
    /// hash as a branch over the last block of the payload, which whoever added the record chose.
    /// Making that block a node with an empty child takes about 2^56 permutations, well below the
    /// strength of the hash. Nothing can be opened below a leaf this way, and no node equals one.
    #[inline(always)]
    fn leaf(key_hash: &Digest, value_hash: &Digest) -> Digest {
        Self::sponge_state(&Self::capacity(tag::JMT_LEAF), &[key_hash, value_hash]).1
    }

    /// `jmt` walks bytes most significant bit first, and each element is stored little-endian.
    #[inline(always)]
    fn path_key(digest: &Digest) -> Digest {
        Digest(digest.0.map(u32::swap_bytes))
    }
}

/// `Trunc8(P(tag))` for `tag::EMPTY`. A test recomputes it.
pub const POSEIDON_EMPTY: Digest = Digest([
    1375433963, 1352261097, 1300241724, 1738100287, 2009413268, 853123064, 487961214, 1116675084,
]);

//! `jmt`'s byte-stream hasher trait, over each hash profile.

use core::{marker::PhantomData, mem::MaybeUninit};

use jmt::{
    SimpleHasher,
    proof::{INTERNAL_DOMAIN_SEPARATOR, LEAF_DOMAIN_SEPARATOR},
};

use crate::hash::{Digest, Hasher, JmtHasher, PermBackend, Poseidon2, Sha256Raw, ShaBackend};

/// Bytes waiting to be hashed. Tree nodes fit inline, so only long values allocate.
struct Buffer {
    used: usize,
    inline: MaybeUninit<[u8; 96]>,
    heap: Vec<u8>,
}

impl Buffer {
    #[inline(always)]
    fn new() -> Self {
        Self {
            used: 0,
            inline: MaybeUninit::uninit(),
            heap: Vec::new(),
        }
    }

    #[inline(always)]
    fn push(&mut self, data: &[u8]) {
        if self.heap.is_empty() && self.used + data.len() <= 96 {
            // SAFETY: the check above keeps the copy inside the inline array.
            unsafe {
                self.inline
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(self.used)
                    .copy_from_nonoverlapping(data.as_ptr(), data.len());
            }
            self.used += data.len();
        } else {
            if self.heap.is_empty() {
                self.heap = self.as_slice().to_vec();
            }
            self.heap.extend_from_slice(data);
        }
    }

    #[inline(always)]
    fn as_slice(&self) -> &[u8] {
        if self.heap.is_empty() {
            // SAFETY: the first `used` bytes were written by `push`.
            unsafe { core::slice::from_raw_parts(self.inline.as_ptr().cast(), self.used) }
        } else {
            &self.heap
        }
    }
}

/// Raw SHA-256 behind `jmt`'s byte stream. Standard SHA-256, so it gives the same roots as `sha2`.
/// The stream hides the word structure, so every node needs two message schedules.
pub struct Sha256Stream<B> {
    buffer: Buffer,
    _backend: PhantomData<B>,
}

impl<B: ShaBackend> SimpleHasher for Sha256Stream<B> {
    #[inline(always)]
    fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            _backend: PhantomData,
        }
    }

    #[inline(always)]
    fn update(&mut self, data: &[u8]) {
        self.buffer.push(data);
    }

    #[inline(always)]
    fn finalize(self) -> [u8; 32] {
        Sha256Raw::<B>::export(&Sha256Raw::<B>::bytes(self.buffer.as_slice()))
    }
}

/// The `sha2` crate behind `jmt`'s byte stream, counting blocks on the host. A guest uses
/// `sha2::Sha256` directly, through `jmt`'s own implementation.
pub struct Sha2Counted<B> {
    hasher: sha2::Sha256,
    length: u64,
    _backend: PhantomData<B>,
}

impl<B: ShaBackend> SimpleHasher for Sha2Counted<B> {
    fn new() -> Self {
        Self {
            hasher: sha2::Digest::new(),
            length: 0,
            _backend: PhantomData,
        }
    }

    fn update(&mut self, data: &[u8]) {
        sha2::Digest::update(&mut self.hasher, data);
        self.length += data.len() as u64;
    }

    fn finalize(self) -> [u8; 32] {
        B::note_crate_blocks((self.length + 8) / 64 + 1);
        sha2::Digest::finalize(self.hasher).into()
    }
}

/// Poseidon2 as SP1's byte sponge. This is the simple way to give `jmt` a Poseidon2 hasher.
pub struct PoseidonSponge<B> {
    buffer: Buffer,
    _backend: PhantomData<B>,
}

impl<B: PermBackend + ShaBackend> SimpleHasher for PoseidonSponge<B> {
    #[inline(always)]
    fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            _backend: PhantomData,
        }
    }

    #[inline(always)]
    fn update(&mut self, data: &[u8]) {
        self.buffer.push(data);
    }

    #[inline(always)]
    fn finalize(self) -> [u8; 32] {
        Poseidon2::<B>::export(&Poseidon2::<B>::byte_sponge(self.buffer.as_slice()))
    }
}

/// Poseidon2 with field-native tree nodes, without forking `jmt`.
///
/// `jmt` hashes a node as exactly three updates: a public separator, then two 32-byte children. This
/// hasher recognizes that shape and hashes the children natively. Anything else, including a value
/// that happens to equal a separator, goes through the byte sponge over all its bytes.
pub struct PoseidonNode<B> {
    updates: u8,
    shape: Shape,
    children: [MaybeUninit<Digest>; 2],
    buffer: Buffer,
    _backend: PhantomData<B>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Unknown,
    Internal,
    Leaf,
    Bytes,
}

impl<B> PoseidonNode<B> {
    /// Falls back to bytes, moving what the node shape has held so far into the buffer.
    #[cold]
    fn fall_back(&mut self) {
        let separator: &[u8] = match self.shape {
            Shape::Internal => INTERNAL_DOMAIN_SEPARATOR,
            Shape::Leaf => LEAF_DOMAIN_SEPARATOR,
            Shape::Unknown | Shape::Bytes => return,
        };
        self.buffer.push(separator);
        for child in &self.children[..self.updates.saturating_sub(1) as usize] {
            // SAFETY: the first `updates - 1` children were written by `update`.
            let bytes = unsafe { &*child.as_ptr().cast::<[u8; 32]>() };
            self.buffer.push(bytes);
        }
    }
}

impl<B: PermBackend + ShaBackend> SimpleHasher for PoseidonNode<B> {
    #[inline(always)]
    fn new() -> Self {
        Self {
            updates: 0,
            shape: Shape::Unknown,
            children: [MaybeUninit::uninit(); 2],
            buffer: Buffer::new(),
            _backend: PhantomData,
        }
    }

    #[inline(always)]
    fn update(&mut self, data: &[u8]) {
        match (self.updates, self.shape) {
            (0, _) if data == INTERNAL_DOMAIN_SEPARATOR => self.shape = Shape::Internal,
            (0, _) if data == LEAF_DOMAIN_SEPARATOR => self.shape = Shape::Leaf,
            (1 | 2, Shape::Internal | Shape::Leaf) if data.len() == 32 => {
                // SAFETY: the slot is 32 bytes and `data` is 32 bytes. Any bytes are a valid digest.
                unsafe {
                    self.children[self.updates as usize - 1]
                        .as_mut_ptr()
                        .cast::<u8>()
                        .copy_from_nonoverlapping(data.as_ptr(), 32);
                }
            }
            _ => {
                self.fall_back();
                self.shape = Shape::Bytes;
                self.buffer.push(data);
            }
        }
        self.updates = self.updates.saturating_add(1);
    }

    #[inline(always)]
    fn finalize(mut self) -> [u8; 32] {
        let digest = match (self.shape, self.updates) {
            (Shape::Internal, 3) => {
                // SAFETY: three updates in this shape wrote both children.
                let (left, right) = unsafe {
                    (
                        self.children[0].assume_init_ref(),
                        self.children[1].assume_init_ref(),
                    )
                };
                Poseidon2::<B>::internal(left, right)
            }
            (Shape::Leaf, 3) => {
                // SAFETY: as above.
                let (key, value) = unsafe {
                    (
                        self.children[0].assume_init_ref(),
                        self.children[1].assume_init_ref(),
                    )
                };
                <Poseidon2<B> as JmtHasher>::leaf(key, value)
            }
            _ => {
                self.fall_back();
                self.shape = Shape::Bytes;
                Poseidon2::<B>::byte_sponge(self.buffer.as_slice())
            }
        };
        Poseidon2::<B>::export(&digest)
    }
}

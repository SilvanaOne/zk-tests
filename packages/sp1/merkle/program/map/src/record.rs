//! The records a batch adds or changes, the witness layout, and the public values.
//!
//! A witness is one 8-aligned buffer: a fixed header, the records, the old payloads, then whatever
//! the engine needs. Nothing here is trusted: roots are checked against the proofs, and every value
//! a write depends on is checked against the state.

use crate::hash::{Digest, Hasher};

pub const ABI_VERSION: u32 = 1;
/// The encoding of records, payloads and batch commitments.
pub const ENCODING_VERSION: u32 = 1;

pub const OP_INSERT: u8 = 1;
pub const OP_UPDATE: u8 = 2;

/// Keys are sequence numbers rather than hashes. Only the indexed map uses them unhashed.
pub const FLAG_SEQUENTIAL: u32 = 1;
/// The batch commitment is one a verifier outside the proof can recompute from the records.
pub const FLAG_EXTERNAL_BATCH_HASH: u32 = 2;
/// Stock Jellyfish arms also verify a separate read proof per key.
pub const FLAG_READ_PROOFS: u32 = 4;

/// Run everything. The other stops end the guest early, to measure each phase by difference.
pub const STOP_NONE: u32 = 0;
pub const STOP_AFTER_DERIVE: u32 = 1;
pub const STOP_AFTER_DECODE: u32 = 2;

/// Identifies the tree in the public values.
pub const TREE_IMM: u32 = 1;
pub const TREE_JMT: u32 = 2;

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Header {
    pub abi_version: u32,
    pub records: u32,
    pub flags: u32,
    pub key_size: u32,
    pub value_size: u32,
    pub stop_after: u32,
    pub height: u32,
    pub sub_batches: u32,
    pub old_len: u64,
    pub records_bytes: u64,
    pub olds_bytes: u64,
    pub engine_bytes: u64,
    pub old_root: Digest,
    /// The claimed new root. Only stock Jellyfish arms use it, and they check it.
    pub new_root: Digest,
    /// The indexed map's root before the length is bound in.
    pub old_internal: Digest,
}

pub const HEADER_BYTES: usize = size_of::<Header>();

/// The words a record's fields take.
pub const fn padded(size: usize) -> usize {
    size.next_multiple_of(8)
}

/// A witness split into its regions.
pub struct Witness<'a> {
    pub header: &'a Header,
    pub records: &'a [u8],
    pub olds: &'a [u8],
    pub engine: &'a [u8],
}

impl<'a> Witness<'a> {
    pub fn parse(bytes: &'a [u8]) -> Self {
        assert!(bytes.as_ptr().addr().is_multiple_of(8), "unaligned witness");
        assert!(bytes.len() >= HEADER_BYTES, "witness too short");
        // SAFETY: the buffer is aligned and long enough, and any bytes are a valid header.
        let header = unsafe { &*bytes.as_ptr().cast::<Header>() };
        assert_eq!(header.abi_version, ABI_VERSION, "wrong ABI version");

        let (records, rest) = bytes[HEADER_BYTES..].split_at(header.records_bytes as usize);
        let (olds, engine) = rest.split_at(header.olds_bytes as usize);
        assert_eq!(
            engine.len() as u64,
            header.engine_bytes,
            "wrong witness size"
        );
        assert!(records.len().is_multiple_of(8) && olds.len().is_multiple_of(8));
        assert_eq!(
            Some(records.len()),
            (header.records as usize).checked_mul(record_stride(header)),
            "wrong record count"
        );
        assert!(
            olds.len()
                .is_multiple_of(padded(header.value_size as usize)),
            "partial old payload"
        );
        assert!(header.stop_after <= STOP_AFTER_DECODE, "unknown stop point");
        Self {
            header,
            records,
            olds,
            engine,
        }
    }

    pub fn records(&self) -> impl Iterator<Item = Record<'a>> + use<'a> {
        let (key_size, value_size) = (
            self.header.key_size as usize,
            self.header.value_size as usize,
        );
        self.records
            .chunks_exact(record_stride(self.header))
            .map(move |bytes| {
                let meta = u64::from_le_bytes(bytes[..8].try_into().unwrap());
                let (key, arg) = bytes[8..].split_at(padded(key_size));
                Record {
                    op: meta as u8,
                    hint: (meta >> 8) as usize,
                    key: &key[..key_size],
                    arg: &arg[..value_size],
                }
            })
    }

    /// The old payloads, in the order the engine consumes them.
    pub fn olds(&self) -> impl Iterator<Item = &'a [u8]> + use<'a> {
        self.olds
            .chunks_exact(padded(self.header.value_size as usize))
    }
}

fn record_stride(header: &Header) -> usize {
    8 + padded(header.key_size as usize) + padded(header.value_size as usize)
}

/// One record, borrowed from the witness.
#[derive(Clone, Copy)]
pub struct Record<'a> {
    pub op: u8,
    /// Where the indexed map's batch verifier finds this record's leaf. Checked, never trusted.
    pub hint: usize,
    /// The preimage of the key, or a sequence number.
    pub key: &'a [u8],
    /// The new payload for an insert, or the amount to add for an update.
    pub arg: &'a [u8],
}

/// The payload's fixed start: an amount as `u128` and a nonce as `u64`, little-endian. Any further
/// bytes are opaque, so a test can vary the record size.
pub const PAYLOAD_MIN: usize = 24;

/// Applies an update: a checked add and a nonce increment, so every write derives from a verified
/// read.
#[inline(always)]
pub fn apply_update(payload: &mut [u8], arg: &[u8]) {
    let amount = u128::from_le_bytes(payload[..16].try_into().unwrap());
    let delta = u128::from_le_bytes(arg[..16].try_into().unwrap());
    let nonce = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    let amount = amount.checked_add(delta).expect("amount overflow");
    let nonce = nonce.checked_add(1).expect("nonce overflow");
    payload[..16].copy_from_slice(&amount.to_le_bytes());
    payload[16..24].copy_from_slice(&nonce.to_le_bytes());
}

/// The start of the batch commitment, which binds the version and the record count.
#[inline(always)]
pub fn batch_seed(state_version: u32, records: u32) -> Digest {
    let mut words = [0u32; 8];
    words[0] = ABI_VERSION;
    words[1] = state_version & 0xff_ffff;
    words[2] = records;
    Digest(words)
}

/// Adds one record to the batch commitment.
#[inline(always)]
pub fn batch_step<H: Hasher>(
    flags: u32,
    prev: &Digest,
    record: &Record,
    key: &Digest,
    arg: &Digest,
) -> Digest {
    let tag = record.op as u32;
    if flags & FLAG_EXTERNAL_BATCH_HASH != 0 {
        H::chain_external(prev, tag, key, arg, record)
    } else {
        H::chain(prev, tag, key, arg)
    }
}

/// The bytes of the finished batch commitment.
pub fn batch_hash_bytes<H: Hasher>(flags: u32, batch_hash: &Digest) -> [u8; 32] {
    if flags & FLAG_EXTERNAL_BATCH_HASH != 0 {
        H::export_external(batch_hash)
    } else {
        H::export(batch_hash)
    }
}

pub const fn state_version(tree: u32, hash_id: u8) -> u32 {
    (tree << 16) | ((hash_id as u32) << 8) | ENCODING_VERSION
}

/// What a proof commits to: everything that defines the transition. The same 152 bytes in every
/// guest, which is three SHA-256 blocks in SP1's public-values hash.
///
/// The witness sets the stop point, flags and sizes, and nothing else constrains them, so they are
/// committed: a verifier must see them to know what was proved. A proof with a non-zero `stop_after`
/// is only a measurement, and proves no transition.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicValues {
    pub abi_version: u32,
    pub state_version: u32,
    pub height: u32,
    pub stop_after: u32,
    pub flags: u32,
    pub key_size: u32,
    pub value_size: u32,
    pub reserved: u32,
    pub old_len: u64,
    pub new_len: u64,
    pub batch_len: u64,
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub batch_hash: [u8; 32],
}

impl PublicValues {
    /// The values known before any verification. The new root and length stay zero until the engine
    /// has verified the transition, so stopping early can never commit an unverified root.
    pub fn start(header: &Header, state_version: u32, old_root: [u8; 32]) -> Self {
        Self {
            abi_version: ABI_VERSION,
            state_version,
            stop_after: header.stop_after,
            flags: header.flags,
            key_size: header.key_size,
            value_size: header.value_size,
            batch_len: header.records as u64,
            old_root,
            ..Default::default()
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: the struct is `repr(C)` with no padding: 32 + 24 + 96 bytes.
        unsafe { core::slice::from_raw_parts((self as *const Self).cast(), size_of::<Self>()) }
    }
}

/// Reads the digests of an aligned region without copying.
#[inline(always)]
pub fn digests(bytes: &[u8]) -> &[Digest] {
    assert!(bytes.as_ptr().addr().is_multiple_of(8), "unaligned digests");
    assert!(bytes.len().is_multiple_of(32), "partial digest");
    // SAFETY: the region is aligned and whole, and any 32 bytes are a valid digest.
    unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len() / 32) }
}

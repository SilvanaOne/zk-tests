//! A whole batch as one witness: every touched leaf and every sibling once.
//!
//! One walk over the tree recomputes the old root and the new root together, from the same siblings,
//! so the old root proves the witness and the new root follows from the guest's own changes. New
//! leaves fill slots from the old length on, and the old root proves those slots were empty, because
//! the walk computes `EMPTY` there instead of reading anything.
//!
//! Engine region, for each sub-batch: a block header, the touched leaves, then the siblings.

use crate::{
    hash::{Digest, ImmHasher},
    imm::{chained::derive_key, types::Leaf},
    record::{
        OP_INSERT, OP_UPDATE, PublicValues, STOP_AFTER_DECODE, STOP_AFTER_DERIVE, TREE_IMM,
        Witness, apply_update, batch_hash_bytes, batch_seed, batch_step, digests, state_version,
    },
};

/// A leaf the batch reads or changes, as it was before the batch.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default)]
pub struct TouchedLeaf {
    pub index: u64,
    pub leaf: Leaf,
}

/// The counts at the start of each sub-batch's engine region.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default)]
pub struct BlockHeader {
    pub records: u64,
    pub touched: u64,
    pub siblings: u64,
}

/// One entry per leaf, so every change to a leaf lands in one place, whichever record made it.
struct Entry<'a> {
    leaf: Leaf,
    /// The leaf's hash before the batch. `EMPTY` for a new leaf.
    old_hash: Digest,
    dirty: bool,
    /// Where the current payload sits in the arena, once an update has changed it.
    payload: Option<usize>,
    /// The payload of a leaf the batch itself inserted, in case a later record updates it.
    inserted: Option<&'a [u8]>,
}

struct Walk<'a, H> {
    n_old: u64,
    n_new: u64,
    touched: &'a [TouchedLeaf],
    entries: &'a [Entry<'a>],
    arena: &'a [u8],
    value_size: usize,
    siblings: core::slice::Iter<'a, Digest>,
    _hasher: core::marker::PhantomData<H>,
}

impl<H: ImmHasher> Walk<'_, H> {
    /// A leaf's new hash. Its value is hashed once here, whatever number of updates changed it.
    fn new_leaf_hash(&self, entry: &Entry) -> Digest {
        match entry.payload {
            Some(offset) => {
                let value = H::bytes(&self.arena[offset..offset + self.value_size]);
                H::leaf(&entry.leaf.key, &value, &entry.leaf.next_key)
            }
            None => entry.leaf.hash::<H>(),
        }
    }

    /// Returns the subtree's old hash, new hash, and whether the batch changed it.
    fn walk(&mut self, level: u32, index: u64, lo: usize, hi: usize) -> (Digest, Digest, bool) {
        let start = index << level;
        let end = start + (1u64 << level);
        if lo == hi && start < self.n_old && (end <= self.n_old || self.n_new == self.n_old) {
            let sibling = *self.siblings.next().expect("missing sibling");
            return (sibling, sibling, false);
        }
        if start >= self.n_new {
            return (H::EMPTY, H::EMPTY, false);
        }
        if level == 0 {
            if lo < hi {
                assert_eq!(self.touched[lo].index, index, "touched leaf out of place");
                let entry = &self.entries[lo];
                let new = if entry.dirty {
                    self.new_leaf_hash(entry)
                } else {
                    entry.old_hash
                };
                return (entry.old_hash, new, entry.dirty);
            }
            let entry = &self.entries[self.touched.len() + (index - self.n_old) as usize];
            return (H::EMPTY, self.new_leaf_hash(entry), true);
        }
        let boundary = start + (1u64 << (level - 1));
        let mid = lo + self.touched[lo..hi].partition_point(|leaf| leaf.index < boundary);
        let (old_left, new_left, changed_left) = self.walk(level - 1, 2 * index, lo, mid);
        let (old_right, new_right, changed_right) = self.walk(level - 1, 2 * index + 1, mid, hi);
        let old = H::node_or_empty(&old_left, &old_right);
        if changed_left || changed_right {
            (old, H::node_or_empty(&new_left, &new_right), true)
        } else {
            (old, old, false)
        }
    }
}

/// Reads a `T` from the start of an aligned region, and returns the rest.
fn split_header<T: Copy>(bytes: &[u8]) -> (T, &[u8]) {
    let (head, rest) = bytes.split_at(size_of::<T>());
    assert!(head.as_ptr().addr().is_multiple_of(8), "unaligned region");
    // SAFETY: the region is aligned and long enough, and `T` is plain integers and digests.
    (unsafe { head.as_ptr().cast::<T>().read() }, rest)
}

pub fn run<H: ImmHasher>(bytes: &[u8]) -> PublicValues {
    let witness = Witness::parse(bytes);
    let header = witness.header;
    assert!((1..=63).contains(&header.height), "bad height");
    let depth = header.height - 1;
    let capacity = 1u64 << depth;
    assert!(
        header.old_len >= 1 && header.old_len <= capacity,
        "bad length"
    );
    assert!(
        H::bind(&header.old_internal, header.old_len) == header.old_root,
        "wrong old root"
    );

    let version = state_version(TREE_IMM, H::ID);
    let mut output = PublicValues {
        height: header.height,
        old_len: header.old_len,
        ..PublicValues::start(header, version, H::export(&header.old_root))
    };

    let value_size = header.value_size as usize;
    let mut batch_hash = batch_seed(version, header.records);
    let mut records = witness.records();
    let mut olds = witness.olds();
    let mut engine = witness.engine;
    let mut root = header.old_internal;
    let mut length = header.old_len;
    let mut arena: Vec<u8> = Vec::new();

    for _ in 0..header.sub_batches {
        let (block, rest) = split_header::<BlockHeader>(engine);
        let touched_bytes = (block.touched as usize)
            .checked_mul(size_of::<TouchedLeaf>())
            .expect("too many touched leaves");
        let sibling_bytes = (block.siblings as usize)
            .checked_mul(size_of::<Digest>())
            .expect("too many siblings");
        let (touched, rest) = rest.split_at(touched_bytes);
        let (sibling_bytes, rest) = rest.split_at(sibling_bytes);
        engine = rest;
        assert!(
            touched.as_ptr().addr().is_multiple_of(8),
            "unaligned leaves"
        );
        // SAFETY: the region is aligned and exactly this long, and any bytes are valid here.
        let touched: &[TouchedLeaf] =
            unsafe { core::slice::from_raw_parts(touched.as_ptr().cast(), block.touched as usize) };
        let siblings = digests(sibling_bytes);
        assert!(
            touched.windows(2).all(|pair| pair[0].index < pair[1].index)
                && touched.last().is_none_or(|leaf| leaf.index < length),
            "touched leaves out of order"
        );
        if header.stop_after == STOP_AFTER_DECODE {
            continue;
        }

        // Old hashes are taken before anything changes.
        let mut entries: Vec<Entry> = Vec::with_capacity(touched.len() + block.records as usize);
        for leaf in touched {
            entries.push(Entry {
                leaf: leaf.leaf,
                old_hash: leaf.leaf.hash::<H>(),
                dirty: false,
                payload: None,
                inserted: None,
            });
        }

        let mut next_length = length;
        for _ in 0..block.records {
            let record = records.next().expect("missing record");
            let key = derive_key::<H>(header.flags, &record);
            let arg = H::bytes(record.arg);
            batch_hash = batch_step::<H>(header.flags, &batch_hash, &record, &key, &arg);
            if header.stop_after == STOP_AFTER_DERIVE {
                continue;
            }

            // The hint must already exist, so a record cannot lean on a later one.
            match record.op {
                OP_INSERT => {
                    assert!(next_length < capacity, "tree full");
                    let low = &mut entries[record.hint];
                    assert!(
                        low.leaf.key < key
                            && (low.leaf.next_key == Digest::ZERO || key < low.leaf.next_key),
                        "wrong low leaf"
                    );
                    let next_key = low.leaf.next_key;
                    low.leaf.next_key = key;
                    low.dirty = true;
                    entries.push(Entry {
                        leaf: Leaf {
                            key,
                            value: arg,
                            next_key,
                        },
                        old_hash: H::EMPTY,
                        dirty: true,
                        payload: None,
                        inserted: Some(record.arg),
                    });
                    next_length += 1;
                }
                OP_UPDATE => {
                    let entry = &mut entries[record.hint];
                    assert!(entry.leaf.key == key, "update through another key's leaf");
                    let offset = match entry.payload {
                        Some(offset) => offset,
                        None => {
                            let old = match entry.inserted {
                                Some(payload) => payload,
                                None => {
                                    let old = olds.next().expect("missing old payload");
                                    let old = &old[..value_size];
                                    assert!(H::bytes(old) == entry.leaf.value, "wrong old value");
                                    old
                                }
                            };
                            let offset = arena.len();
                            arena.extend_from_slice(old);
                            entry.payload = Some(offset);
                            offset
                        }
                    };
                    apply_update(&mut arena[offset..offset + value_size], record.arg);
                    entry.dirty = true;
                }
                op => panic!("bad operation {op}"),
            }
        }
        if header.stop_after == STOP_AFTER_DERIVE {
            continue;
        }

        let mut walk = Walk::<H> {
            n_old: length,
            n_new: next_length,
            touched,
            entries: &entries,
            arena: &arena,
            value_size,
            siblings: siblings.iter(),
            _hasher: core::marker::PhantomData,
        };
        let (old, new, _) = walk.walk(depth, 0, 0, touched.len());
        assert!(walk.siblings.next().is_none(), "trailing siblings");
        assert!(old == root, "wrong old root");
        root = new;
        length = next_length;
        arena.clear();
    }
    assert!(engine.is_empty(), "trailing witness");

    output.batch_hash = batch_hash_bytes::<H>(header.flags, &batch_hash);
    if header.stop_after != 0 {
        return output;
    }
    assert!(records.next().is_none(), "trailing records");
    assert!(olds.next().is_none(), "trailing old payloads");
    output.new_len = length;
    output.new_root = H::export(&H::bind(&root, length));
    output
}

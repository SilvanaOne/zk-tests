//! A whole batch as one witness over the same tree, with every touched node once.
//!
//! The witness is the part of the tree the batch touches, in pre-order, with everything else as an
//! opaque hash. The walk follows each key's own path bits, so a key can only reach the leaf or empty
//! slot on its path, and any record that would need to go into an opaque hash aborts. The tree has a
//! leaf at the shortest prefix that is unique to it, an empty range is the placeholder, and anything
//! else is an internal node, so this reproduces `jmt`'s roots without its 16-ary nodes.
//!
//! Engine region: the number of tag bytes and of digests, the tags (padded to 8 bytes), the digests.

use crate::{
    hash::{Digest, JmtHasher},
    record::{
        OP_INSERT, OP_UPDATE, PublicValues, Record, STOP_AFTER_DECODE, STOP_AFTER_DERIVE, TREE_JMT,
        Witness, apply_update, batch_hash_bytes, batch_seed, batch_step, digests, padded,
        state_version,
    },
};

pub const TAG_NULL: u8 = 0;
pub const TAG_HASH: u8 = 1;
pub const TAG_LEAF: u8 = 2;
pub const TAG_BRANCH: u8 = 3;

const PATH_BITS: u32 = 256;

struct Op<'a> {
    path: Digest,
    key: Digest,
    sequence: u32,
    record: Record<'a>,
}

/// A leaf in a subtree being rebuilt.
struct NewLeaf {
    path: Digest,
    key: Digest,
    value: Digest,
}

#[inline(always)]
fn path_bit(path: &Digest, depth: u32) -> bool {
    (path.0[depth as usize / 32] >> (31 - depth % 32)) & 1 == 1
}

fn common_prefix_bits(a: &Digest, b: &Digest) -> u32 {
    for (index, (x, y)) in a.0.iter().zip(&b.0).enumerate() {
        if x != y {
            return index as u32 * 32 + (x ^ y).leading_zeros();
        }
    }
    PATH_BITS
}

struct Walk<'a, H, I> {
    tags: core::slice::Iter<'a, u8>,
    digests: core::slice::Iter<'a, Digest>,
    olds: I,
    value_size: usize,
    payload: Vec<u8>,
    _hasher: core::marker::PhantomData<H>,
}

impl<'a, H: JmtHasher, I: Iterator<Item = &'a [u8]>> Walk<'a, H, I> {
    fn next_digest(&mut self) -> Digest {
        *self.digests.next().expect("missing digest")
    }

    /// Returns the subtree's old hash and new hash.
    fn walk(&mut self, depth: u32, ops: &[Op]) -> (Digest, Digest) {
        match *self.tags.next().expect("missing tag") {
            TAG_NULL => {
                if ops.is_empty() {
                    (H::PLACEHOLDER, H::PLACEHOLDER)
                } else {
                    let leaves = self.merge(None, ops);
                    (H::PLACEHOLDER, Self::build(depth, &leaves))
                }
            }
            TAG_HASH => {
                assert!(ops.is_empty(), "a record leads into an opaque hash");
                let hash = self.next_digest();
                (hash, hash)
            }
            TAG_LEAF => {
                let leaf_key = self.next_digest();
                let leaf_value = self.next_digest();
                let old = H::leaf(&leaf_key, &leaf_value);
                if ops.is_empty() {
                    return (old, old);
                }
                let leaf_path = H::path_key(&leaf_key);
                assert!(
                    common_prefix_bits(&leaf_path, &ops[0].path) >= depth,
                    "the records do not belong under this leaf"
                );
                let leaves = self.merge(Some((leaf_path, leaf_key, leaf_value)), ops);
                (old, Self::build(depth, &leaves))
            }
            TAG_BRANCH => {
                assert!(depth < PATH_BITS, "path too long");
                let split = ops.partition_point(|op| !path_bit(&op.path, depth));
                let (old_left, new_left) = self.walk(depth + 1, &ops[..split]);
                let (old_right, new_right) = self.walk(depth + 1, &ops[split..]);
                let old = H::internal(&old_left, &old_right);
                if ops.is_empty() {
                    (old, old)
                } else {
                    (old, H::internal(&new_left, &new_right))
                }
            }
            tag => panic!("bad tag {tag}"),
        }
    }

    /// Applies each key's records in order, keeps an unrelated existing leaf, and returns the leaves
    /// sorted by path. Each key's final value is hashed once.
    fn merge(&mut self, existing: Option<(Digest, Digest, Digest)>, ops: &[Op]) -> Vec<NewLeaf> {
        let mut leaves = Vec::with_capacity(ops.len() + 1);
        let mut existing = existing;
        for run in ops.chunk_by(|a, b| a.key == b.key) {
            let path = run[0].path;
            // An unrelated existing leaf goes in path order.
            if let Some((leaf_path, leaf_key, leaf_value)) = existing
                && leaf_path < path
            {
                leaves.push(NewLeaf {
                    path: leaf_path,
                    key: leaf_key,
                    value: leaf_value,
                });
                existing = None;
            }
            let current = match existing {
                Some((leaf_path, _, leaf_value)) if leaf_path == path => {
                    existing = None;
                    Some(leaf_value)
                }
                _ => None,
            };

            let mut started = false;
            for op in run {
                match op.record.op {
                    OP_INSERT => {
                        assert!(current.is_none() && !started, "insert of an existing key");
                        self.payload.clear();
                        self.payload.extend_from_slice(op.record.arg);
                    }
                    OP_UPDATE => {
                        if !started {
                            let old_value = current.expect("update of an absent key");
                            let old = self.olds.next().expect("missing old payload");
                            let old = &old[..self.value_size];
                            assert!(H::bytes(old) == old_value, "wrong old value");
                            self.payload.clear();
                            self.payload.extend_from_slice(old);
                        }
                        apply_update(&mut self.payload, op.record.arg);
                    }
                    other => panic!("bad operation {other}"),
                }
                started = true;
            }
            leaves.push(NewLeaf {
                path,
                key: run[0].key,
                value: H::bytes(&self.payload),
            });
        }
        if let Some((path, key, value)) = existing {
            leaves.push(NewLeaf { path, key, value });
        }
        leaves
    }

    /// The hash of a subtree holding exactly these leaves.
    fn build(depth: u32, leaves: &[NewLeaf]) -> Digest {
        match leaves {
            [] => H::PLACEHOLDER,
            [leaf] => H::leaf(&leaf.key, &leaf.value),
            _ => {
                assert!(depth < PATH_BITS, "path too long");
                let split = leaves.partition_point(|leaf| !path_bit(&leaf.path, depth));
                H::internal(
                    &Self::build(depth + 1, &leaves[..split]),
                    &Self::build(depth + 1, &leaves[split..]),
                )
            }
        }
    }
}

pub fn run<H: JmtHasher>(bytes: &[u8]) -> PublicValues {
    let witness = Witness::parse(bytes);
    let header = witness.header;
    let version = state_version(TREE_JMT, H::ID);
    let mut output = PublicValues::start(header, version, H::export(&header.old_root));

    let mut batch_hash = batch_seed(version, header.records);
    let mut ops = Vec::with_capacity(header.records as usize);
    for (sequence, record) in witness.records().enumerate() {
        let key = H::bytes(record.key);
        let arg = H::bytes(record.arg);
        batch_hash = batch_step::<H>(header.flags, &batch_hash, &record, &key, &arg);
        ops.push(Op {
            path: H::path_key(&key),
            key,
            sequence: sequence as u32,
            record,
        });
    }
    output.batch_hash = batch_hash_bytes::<H>(header.flags, &batch_hash);
    if header.stop_after == STOP_AFTER_DERIVE {
        return output;
    }
    ops.sort_unstable_by(|a, b| a.path.cmp(&b.path).then(a.sequence.cmp(&b.sequence)));

    let tag_bytes = u64::from_le_bytes(witness.engine[..8].try_into().unwrap()) as usize;
    let digest_count = u64::from_le_bytes(witness.engine[8..16].try_into().unwrap()) as usize;
    let (tags, digest_bytes) = witness.engine[16..].split_at(padded(tag_bytes));
    assert_eq!(
        Some(digest_bytes.len()),
        digest_count.checked_mul(size_of::<Digest>()),
        "wrong witness size"
    );
    if header.stop_after == STOP_AFTER_DECODE {
        return output;
    }

    let mut walk = Walk::<H, _> {
        tags: tags[..tag_bytes].iter(),
        digests: digests(digest_bytes).iter(),
        olds: witness.olds(),
        value_size: header.value_size as usize,
        payload: Vec::with_capacity(header.value_size as usize),
        _hasher: core::marker::PhantomData,
    };
    let (old, new) = walk.walk(0, &ops);
    assert!(walk.tags.next().is_none(), "trailing tags");
    assert!(walk.digests.next().is_none(), "trailing digests");
    assert!(walk.olds.next().is_none(), "trailing old payloads");
    assert!(old == header.old_root, "wrong old root");
    output.new_root = H::export(&new);
    output
}

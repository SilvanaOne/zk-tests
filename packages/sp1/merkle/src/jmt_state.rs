//! The Jellyfish tree on the host: a small in-memory store and the witnesses it produces.

use std::{collections::HashMap, time::Instant};

use anyhow::{Result, anyhow, bail};
use jmt::{
    JellyfishMerkleTree, KeyHash, OwnedValue, RootHash, SimpleHasher, Version,
    storage::{LeafNode, Node, NodeKey, TreeReader, TreeUpdateBatch},
};
use merkle_map::{
    hash::{Digest, Hasher, JmtHasher},
    jellyfish::batch::{TAG_BRANCH, TAG_HASH, TAG_LEAF, TAG_NULL},
    record::{
        FLAG_EXTERNAL_BATCH_HASH, FLAG_READ_PROOFS, Header, OP_INSERT, OP_UPDATE, apply_update,
    },
};

use crate::{
    point::Point,
    prepare::{Sidecar, assemble},
    workload::{self, Rec, encode_records, initial_payload, key_preimage, pad_extend},
};

/// Nodes and the latest value per key. `MockTreeStore` clones every batch and cannot be exported.
#[derive(Default)]
pub struct Store {
    nodes: HashMap<NodeKey, Node>,
    values: HashMap<KeyHash, OwnedValue>,
}

impl TreeReader for Store {
    fn get_node_option(&self, node_key: &NodeKey) -> Result<Option<Node>> {
        Ok(self.nodes.get(node_key).cloned())
    }

    fn get_value_option(
        &self,
        _max_version: Version,
        key_hash: KeyHash,
    ) -> Result<Option<OwnedValue>> {
        Ok(self.values.get(&key_hash).cloned())
    }

    fn get_rightmost_leaf(&self) -> Result<Option<(NodeKey, LeafNode)>> {
        bail!("only a tree restore needs this")
    }
}

impl Store {
    pub fn apply(&mut self, batch: &TreeUpdateBatch) {
        for (key, node) in batch.node_batch.nodes() {
            self.nodes.insert(key.clone(), node.clone());
        }
        for ((_, key), value) in batch.node_batch.values() {
            match value {
                Some(value) => self.values.insert(*key, value.clone()),
                None => self.values.remove(key),
            };
        }
    }
}

/// A built tree, and the version the next batch would write.
pub struct State<S> {
    pub store: Store,
    pub root: RootHash,
    pub next_version: Version,
    pub build_ms: f64,
    _hasher: core::marker::PhantomData<S>,
}

impl<S: SimpleHasher> State<S> {
    /// Applies a batch's writes, so the next batch starts from them.
    pub fn apply(&mut self, records: &[Rec]) -> Result<()> {
        let collapsed = collapse(records, self)?;
        let (root, batch) = self
            .tree()
            .put_value_set(collapsed.writes, self.next_version)
            .map_err(|error| anyhow!("{error}"))?;
        self.store.apply(&batch);
        self.root = root;
        self.next_version += 1;
        Ok(())
    }

    /// Fills a tree with `count` records in one batch.
    pub fn build(seed: u64, count: u64, key_size: usize, value_size: usize) -> Result<Self> {
        let started = Instant::now();
        let mut store = Store::default();
        let root = if count == 0 {
            JellyfishMerkleTree::<Store, S>::EMPTY_ROOT
        } else {
            let records: Vec<_> = (0..count)
                .map(|index| {
                    (
                        KeyHash(S::hash(key_preimage(seed, index, key_size))),
                        initial_payload(seed, index, value_size),
                    )
                })
                .collect();
            let (roots, batch) = JellyfishMerkleTree::<_, S>::new(&store)
                .batch_put_value_sets(vec![records], None, 0)
                .map_err(|error| anyhow!("{error}"))?;
            store.apply(&batch);
            roots[0]
        };
        Ok(Self {
            store,
            root,
            next_version: if count == 0 { 0 } else { 1 },
            build_ms: started.elapsed().as_secs_f64() * 1e3,
            _hasher: core::marker::PhantomData,
        })
    }

    pub fn tree(&self) -> JellyfishMerkleTree<'_, Store, S> {
        JellyfishMerkleTree::new(&self.store)
    }
}

/// One key's writes collapsed: what the guest will derive for itself.
pub struct Collapsed {
    pub writes: Vec<(KeyHash, Option<OwnedValue>)>,
    /// The old payload per updated key, in key order.
    pub olds: Vec<Vec<u8>>,
    /// The old payload (updates) or nothing (inserts) per key, in key order.
    pub reads: Vec<Option<Vec<u8>>>,
}

pub fn collapse<S: SimpleHasher>(records: &[Rec], state: &State<S>) -> Result<Collapsed> {
    let mut ops: Vec<(KeyHash, usize)> = records
        .iter()
        .enumerate()
        .map(|(sequence, record)| (KeyHash(S::hash(&record.key)), sequence))
        .collect();
    ops.sort_unstable_by_key(|(key, sequence)| (key.0, *sequence));

    let mut collapsed = Collapsed {
        writes: Vec::new(),
        olds: Vec::new(),
        reads: Vec::new(),
    };
    for run in ops.chunk_by(|a, b| a.0 == b.0) {
        let key = run[0].0;
        let first = &records[run[0].1];
        let (mut payload, rest) = match first.op {
            OP_INSERT => {
                if state.store.values.contains_key(&key) {
                    bail!("insert of an existing key");
                }
                collapsed.reads.push(None);
                (first.arg.clone(), &run[1..])
            }
            OP_UPDATE => {
                let old = state
                    .store
                    .values
                    .get(&key)
                    .ok_or_else(|| anyhow!("update of an absent key"))?
                    .clone();
                collapsed.olds.push(old.clone());
                collapsed.reads.push(Some(old.clone()));
                (old, run)
            }
            op => bail!("bad operation {op}"),
        };
        for (_, sequence) in rest {
            apply_update(&mut payload, &records[*sequence].arg);
        }
        collapsed.writes.push((key, Some(payload)));
    }
    Ok(collapsed)
}

/// Splits one serialized update proof into a frame per key.
pub fn frame_update_proof(bytes: &[u8], out: &mut Vec<u8>) -> FrameStats {
    let count = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let mut offset = 4;
    let mut stats = FrameStats::default();
    for _ in 0..count {
        let start = offset;
        offset = proof_end(bytes, offset, &mut stats);
        out.extend_from_slice(&((offset - start) as u32).to_le_bytes());
        out.extend_from_slice(&bytes[start..offset]);
    }
    assert_eq!(offset, bytes.len());
    stats
}

/// The depth of the proofs in a witness.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FrameStats {
    pub proofs: u64,
    pub siblings: u64,
    pub null_siblings: u64,
    pub max_siblings: u64,
    pub with_leaf: u64,
}

/// Walks one Borsh `SparseMerkleProof`: a leaf option, then a list of tagged siblings.
fn proof_end(bytes: &[u8], mut offset: usize, stats: &mut FrameStats) -> usize {
    match bytes[offset] {
        0 => offset += 1,
        1 => {
            offset += 65;
            stats.with_leaf += 1;
        }
        tag => panic!("bad leaf tag {tag}"),
    }
    let siblings = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    offset += 4;
    for _ in 0..siblings {
        match bytes[offset] {
            0 => {
                offset += 1;
                stats.null_siblings += 1;
            }
            1 | 2 => offset += 65,
            tag => panic!("bad sibling tag {tag}"),
        }
    }
    stats.proofs += 1;
    stats.siblings += siblings as u64;
    stats.max_siblings = stats.max_siblings.max(siblings as u64);
    offset
}

/// Writes the witness of a stock arm.
pub fn stock_witness<H: Hasher, S: SimpleHasher>(
    point: &Point,
    state: &State<S>,
) -> Result<(Vec<u8>, Sidecar)> {
    stock_witness_of::<H, S>(point, state, &workload::records(point))
}

/// The same for chosen records.
pub fn stock_witness_of<H: Hasher, S: SimpleHasher>(
    point: &Point,
    state: &State<S>,
    records: &[Rec],
) -> Result<(Vec<u8>, Sidecar)> {
    let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
    let collapsed = collapse(records, state)?;
    let version = state.next_version;

    let started = Instant::now();
    let (no_proof_root, _) = state
        .tree()
        .put_value_set(collapsed.writes.clone(), version)
        .map_err(|error| anyhow!("{error}"))?;
    let apply_ms = started.elapsed().as_secs_f64() * 1e3;

    let started = Instant::now();
    let (new_root, proof, _) = state
        .tree()
        .put_value_set_with_proof(collapsed.writes.clone(), version)
        .map_err(|error| anyhow!("{error}"))?;
    let proof_ms = started.elapsed().as_secs_f64() * 1e3;
    assert_eq!(no_proof_root, new_root);

    let mut engine = Vec::new();
    let stats = frame_update_proof(&borsh::to_vec(&proof)?, &mut engine);

    if point.read_proofs {
        for ((key, _), read) in collapsed.writes.iter().zip(&collapsed.reads) {
            let (value, proof) = state
                .tree()
                .get_with_proof(*key, version - 1)
                .map_err(|error| anyhow!("{error}"))?;
            assert_eq!(value.as_ref(), read.as_ref());
            let bytes = borsh::to_vec(&proof)?;
            engine.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            engine.extend_from_slice(&bytes);
        }
    }
    engine.resize(engine.len().next_multiple_of(8), 0);

    let mut olds = Vec::new();
    for old in &collapsed.olds {
        pad_extend(&mut olds, old, value_size);
    }
    let mut flags = 0;
    if point.external_batch_hash {
        flags |= FLAG_EXTERNAL_BATCH_HASH;
    }
    if point.read_proofs {
        flags |= FLAG_READ_PROOFS;
    }
    let header = Header {
        flags,
        records: records.len() as u32,
        key_size: point.key_size,
        value_size: point.value_size,
        sub_batches: 1,
        old_root: H::import(&state.root.0),
        new_root: H::import(&new_root.0),
        ..Default::default()
    };
    let (witness, mut sidecar) = assemble(
        header,
        &encode_records(records, key_size, value_size),
        &olds,
        &engine,
    );
    sidecar.state_build_ms = state.build_ms;
    sidecar.apply_ms = apply_ms;
    sidecar.witness_ms = proof_ms;
    sidecar.distinct_keys = collapsed.writes.len() as u64;
    sidecar.frames = Some(stats);
    sidecar.expected_new_root = Some(new_root.0);
    Ok((witness, sidecar))
}

/// A sibling as a Borsh `SparseMerkleProof` holds it. The type itself is private to `jmt`.
enum ProofNode {
    Null,
    Internal([u8; 32], [u8; 32]),
    Leaf([u8; 32], [u8; 32]),
}

type ProofLeaf = Option<([u8; 32], [u8; 32])>;

fn pair(bytes: &[u8], offset: usize) -> ([u8; 32], [u8; 32]) {
    (
        bytes[offset..offset + 32].try_into().unwrap(),
        bytes[offset + 32..offset + 64].try_into().unwrap(),
    )
}

/// Reads one Borsh `SparseMerkleProof`. Siblings run from the leaf up to the root.
fn parse_proof(bytes: &[u8]) -> (ProofLeaf, Vec<ProofNode>) {
    let (leaf, mut offset) = match bytes[0] {
        0 => (None, 1),
        1 => (Some(pair(bytes, 1)), 65),
        tag => panic!("bad leaf tag {tag}"),
    };
    let count = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    offset += 4;
    let mut siblings = Vec::with_capacity(count as usize);
    for _ in 0..count {
        match bytes[offset] {
            0 => {
                siblings.push(ProofNode::Null);
                offset += 1;
            }
            1 => {
                let (left, right) = pair(bytes, offset + 1);
                siblings.push(ProofNode::Internal(left, right));
                offset += 65;
            }
            2 => {
                let (key, value) = pair(bytes, offset + 1);
                siblings.push(ProofNode::Leaf(key, value));
                offset += 65;
            }
            tag => panic!("bad sibling tag {tag}"),
        }
    }
    assert_eq!(offset, bytes.len());
    (leaf, siblings)
}

/// The part of the tree a batch touches.
enum Trie {
    Unknown,
    Null,
    Hash(Digest),
    Leaf(Digest, Digest),
    Branch(Box<Trie>, Box<Trie>),
}

fn key_bit(key: &[u8; 32], depth: usize) -> bool {
    (key[depth / 8] >> (7 - depth % 8)) & 1 == 1
}

impl Trie {
    /// Adds one key's path. A position already opened by another key's path is kept.
    fn add_path<H: Hasher + JmtHasher>(
        &mut self,
        depth: usize,
        key: &[u8; 32],
        leaf: &ProofLeaf,
        siblings: &[ProofNode],
    ) {
        if depth == siblings.len() {
            *self = match leaf {
                Some((leaf_key, leaf_value)) => {
                    Trie::Leaf(H::import(leaf_key), H::import(leaf_value))
                }
                None => Trie::Null,
            };
            return;
        }
        if !matches!(self, Trie::Branch(..)) {
            *self = Trie::Branch(Box::new(Trie::Unknown), Box::new(Trie::Unknown));
        }
        let Trie::Branch(left, right) = self else {
            unreachable!("set just above")
        };
        let (next, other) = if key_bit(key, depth) {
            (right, left)
        } else {
            (left, right)
        };
        if matches!(**other, Trie::Unknown) {
            **other = match &siblings[siblings.len() - 1 - depth] {
                ProofNode::Null => Trie::Null,
                ProofNode::Internal(left, right) => Trie::Hash(<H as JmtHasher>::internal(
                    &H::import(left),
                    &H::import(right),
                )),
                ProofNode::Leaf(key, value) => {
                    Trie::Hash(<H as JmtHasher>::leaf(&H::import(key), &H::import(value)))
                }
            };
        }
        next.add_path::<H>(depth + 1, key, leaf, siblings);
    }

    /// Writes the pre-order tags and digests a guest reads.
    fn write(&self, tags: &mut Vec<u8>, digests: &mut Vec<Digest>) {
        match self {
            Trie::Unknown => panic!("a position no proof covered"),
            Trie::Null => tags.push(TAG_NULL),
            Trie::Hash(hash) => {
                tags.push(TAG_HASH);
                digests.push(*hash);
            }
            Trie::Leaf(key, value) => {
                tags.push(TAG_LEAF);
                digests.push(*key);
                digests.push(*value);
            }
            Trie::Branch(left, right) => {
                tags.push(TAG_BRANCH);
                left.write(tags, digests);
                right.write(tags, digests);
            }
        }
    }
}

/// Writes the witness of a batch arm: one partial tree over all the keys.
pub fn batch_witness<H: Hasher + JmtHasher, S: SimpleHasher>(
    point: &Point,
    state: &State<S>,
) -> Result<(Vec<u8>, Sidecar)> {
    batch_witness_of::<H, S>(point, state, &workload::records(point))
}

/// The same for chosen records.
pub fn batch_witness_of<H: Hasher + JmtHasher, S: SimpleHasher>(
    point: &Point,
    state: &State<S>,
    records: &[Rec],
) -> Result<(Vec<u8>, Sidecar)> {
    let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
    let collapsed = collapse(records, state)?;
    let version = state.next_version;

    let started = Instant::now();
    let (new_root, _) = state
        .tree()
        .put_value_set(collapsed.writes.clone(), version)
        .map_err(|error| anyhow!("{error}"))?;
    let apply_ms = started.elapsed().as_secs_f64() * 1e3;

    let started = Instant::now();
    // An empty tree has no version to read proofs from, and needs none.
    let mut trie = if version == 0 {
        Trie::Null
    } else {
        Trie::Unknown
    };
    for (key, _) in collapsed.writes.iter().filter(|_| version > 0) {
        let (_, proof) = state
            .tree()
            .get_with_proof(*key, version - 1)
            .map_err(|error| anyhow!("{error}"))?;
        let (leaf, siblings) = parse_proof(&borsh::to_vec(&proof)?);
        trie.add_path::<H>(0, &key.0, &leaf, &siblings);
    }
    let mut tags = Vec::new();
    let mut digests = Vec::new();
    trie.write(&mut tags, &mut digests);
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut engine = Vec::with_capacity(16 + tags.len() + digests.len() * 32);
    engine.extend_from_slice(&(tags.len() as u64).to_le_bytes());
    engine.extend_from_slice(&(digests.len() as u64).to_le_bytes());
    engine.extend_from_slice(&tags);
    engine.resize(engine.len().next_multiple_of(8), 0);
    for digest in &digests {
        // SAFETY: a digest is 32 bytes with no padding.
        engine.extend_from_slice(unsafe { &*(digest as *const Digest).cast::<[u8; 32]>() });
    }

    let mut olds = Vec::new();
    for old in &collapsed.olds {
        pad_extend(&mut olds, old, value_size);
    }
    let header = Header {
        flags: if point.external_batch_hash {
            FLAG_EXTERNAL_BATCH_HASH
        } else {
            0
        },
        records: records.len() as u32,
        key_size: point.key_size,
        value_size: point.value_size,
        sub_batches: 1,
        old_root: H::import(&state.root.0),
        ..Default::default()
    };
    let (witness, mut sidecar) = assemble(
        header,
        &encode_records(records, key_size, value_size),
        &olds,
        &engine,
    );
    sidecar.state_build_ms = state.build_ms;
    sidecar.apply_ms = apply_ms;
    sidecar.witness_ms = witness_ms;
    sidecar.distinct_keys = collapsed.writes.len() as u64;
    sidecar.expected_new_root = Some(new_root.0);
    sidecar.tags = Some(tags.len() as u64);
    sidecar.digests = Some(digests.len() as u64);
    Ok((witness, sidecar))
}

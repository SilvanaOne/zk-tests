//! The indexed map on the host: its built state, and the witnesses it produces.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    time::Instant,
};

use anyhow::{Result, anyhow, bail};
use merkle_map::{
    hash::{Digest, ImmHasher},
    imm::{
        batch::{BlockHeader, TouchedLeaf},
        host::IndexedMerkleMap,
        types::RecordWitness,
    },
    record::{
        FLAG_EXTERNAL_BATCH_HASH, FLAG_SEQUENTIAL, Header, OP_INSERT, OP_UPDATE, apply_update,
    },
};

use crate::{
    point::{Point, Workload},
    prepare::{Sidecar, assemble},
    workload::{self, Rec, encode_records, initial_payload, key_preimage, pad_extend},
};

/// A built map, and the payloads behind its value hashes.
pub struct ImmState<H> {
    pub map: IndexedMerkleMap<H>,
    pub payloads: HashMap<Digest, Vec<u8>>,
    pub build_ms: f64,
}

fn key_of<H: ImmHasher>(sequential: bool, preimage: &[u8]) -> Digest {
    if sequential {
        H::encode_sequence(u64::from_le_bytes(preimage.try_into().unwrap()))
    } else {
        H::bytes(preimage)
    }
}

fn sequential(point: &Point) -> bool {
    point.workload == Workload::Append
}

impl<H: ImmHasher> ImmState<H> {
    pub fn build(point: &Point) -> Self {
        let started = Instant::now();
        let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
        let sequential = sequential(point);
        let mut payloads = HashMap::with_capacity(point.state as usize);
        let map = IndexedMerkleMap::bulk_load(
            point.height as usize,
            (0..point.state).map(|index| {
                let key = key_of::<H>(sequential, &key_preimage(point.seed, index, key_size));
                let payload = initial_payload(point.seed, index, value_size);
                let value = H::bytes(&payload);
                payloads.insert(key, payload);
                (key, value)
            }),
        );
        Self {
            map,
            payloads,
            build_ms: started.elapsed().as_secs_f64() * 1e3,
        }
    }
}

impl<H: ImmHasher> ImmState<H> {
    /// Applies a batch, so the next batch starts from it.
    pub fn apply(&mut self, point: &Point, records: &[Rec]) -> Result<()> {
        let sequential = sequential(point);
        for record in records {
            let key = key_of::<H>(sequential, &record.key);
            let mut current = HashMap::new();
            apply_record(&mut self.map, &mut current, &self.payloads, key, record)?;
            self.payloads.extend(current);
        }
        Ok(())
    }
}

fn flags(point: &Point) -> u32 {
    let mut flags = 0;
    if sequential(point) {
        flags |= FLAG_SEQUENTIAL;
    }
    if point.external_batch_hash {
        flags |= FLAG_EXTERNAL_BATCH_HASH;
    }
    flags
}

fn header<H: ImmHasher>(point: &Point, map: &IndexedMerkleMap<H>, records: u32) -> Header {
    Header {
        flags: flags(point),
        records,
        key_size: point.key_size,
        value_size: point.value_size,
        height: point.height,
        sub_batches: point.sub_batches,
        old_len: map.length(),
        old_root: map.root(),
        old_internal: map.internal_root(),
        ..Default::default()
    }
}

/// Applies one record to a working copy, returning the old payload for an update.
fn apply_record<H: ImmHasher>(
    map: &mut IndexedMerkleMap<H>,
    current: &mut HashMap<Digest, Vec<u8>>,
    base: &HashMap<Digest, Vec<u8>>,
    key: Digest,
    record: &Rec,
) -> Result<Option<Vec<u8>>> {
    match record.op {
        OP_INSERT => {
            map.insert(key, H::bytes(&record.arg))
                .map_err(|error| anyhow!("{error}"))?;
            current.insert(key, record.arg.clone());
            Ok(None)
        }
        OP_UPDATE => {
            let old = current
                .get(&key)
                .or_else(|| base.get(&key))
                .ok_or_else(|| anyhow!("update of an absent key"))?
                .clone();
            let mut payload = old.clone();
            apply_update(&mut payload, &record.arg);
            map.update(key, H::bytes(&payload))
                .map_err(|error| anyhow!("{error}"))?;
            current.insert(key, payload);
            Ok(Some(old))
        }
        op => bail!("bad operation {op}"),
    }
}

/// One witness per record, each built from the state the previous record left.
pub fn chained_witness<H: ImmHasher>(
    point: &Point,
    state: &ImmState<H>,
) -> Result<(Vec<u8>, Sidecar)> {
    chained_witness_of(point, state, workload::records(point))
}

/// The same for chosen records.
pub fn chained_witness_of<H: ImmHasher>(
    point: &Point,
    state: &ImmState<H>,
    records: Vec<Rec>,
) -> Result<(Vec<u8>, Sidecar)> {
    let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
    let sequential = sequential(point);
    let keys: Vec<Digest> = records
        .iter()
        .map(|record| key_of::<H>(sequential, &record.key))
        .collect();

    let mut map = state.map.clone();
    let mut current = HashMap::new();
    let started = Instant::now();
    for (key, record) in keys.iter().zip(&records) {
        apply_record(&mut map, &mut current, &state.payloads, *key, record)?;
    }
    let apply_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut map = state.map.clone();
    let mut current: HashMap<Digest, Vec<u8>> = HashMap::new();
    let mut olds = Vec::new();
    let mut proofs = Vec::with_capacity(records.len());
    let started = Instant::now();
    for (key, record) in keys.iter().zip(&records) {
        match record.op {
            OP_INSERT => {
                let witness = map
                    .insert_and_generate_witness(*key, H::bytes(&record.arg))
                    .map_err(|error| anyhow!("{error}"))?;
                current.insert(*key, record.arg.clone());
                proofs.push(RecordWitness::Insert(witness));
            }
            OP_UPDATE => {
                let old = current
                    .get(key)
                    .or_else(|| state.payloads.get(key))
                    .ok_or_else(|| anyhow!("update of an absent key"))?
                    .clone();
                pad_extend(&mut olds, &old, value_size);
                let mut payload = old;
                apply_update(&mut payload, &record.arg);
                let witness = map
                    .update_and_generate_witness(*key, H::bytes(&payload))
                    .map_err(|error| anyhow!("{error}"))?;
                current.insert(*key, payload);
                proofs.push(RecordWitness::Update(witness));
            }
            op => bail!("bad operation {op}"),
        }
    }
    let witness_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut engine = borsh::to_vec(&proofs)?;
    engine.resize(engine.len().next_multiple_of(8), 0);
    let (witness, mut sidecar) = assemble(
        header(point, &state.map, records.len() as u32),
        &encode_records(&records, key_size, value_size),
        &olds,
        &engine,
    );
    sidecar.distinct_keys = distinct_keys(&records);
    sidecar.state_build_ms = state.build_ms;
    sidecar.apply_ms = apply_ms;
    sidecar.witness_ms = witness_ms;
    sidecar.expected_new_root = Some(H::export(&map.root()));
    Ok((witness, sidecar))
}

/// One witness per block of the batch.
pub fn batch_witness<H: ImmHasher>(
    point: &Point,
    state: &ImmState<H>,
) -> Result<(Vec<u8>, Sidecar)> {
    batch_witness_of(point, state, workload::records(point))
}

/// The same for chosen records.
pub fn batch_witness_of<H: ImmHasher>(
    point: &Point,
    state: &ImmState<H>,
    mut records: Vec<Rec>,
) -> Result<(Vec<u8>, Sidecar)> {
    let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
    let sequential = sequential(point);
    let keys: Vec<Digest> = records
        .iter()
        .map(|record| key_of::<H>(sequential, &record.key))
        .collect();

    let mut map = state.map.clone();
    let mut current: HashMap<Digest, Vec<u8>> = HashMap::new();
    let mut olds = Vec::new();
    let mut engine = Vec::new();
    let (mut apply_ms, mut witness_ms) = (0.0, 0.0);
    let per_block = records
        .len()
        .div_ceil(point.sub_batches.max(1) as usize)
        .max(1);
    let mut block_start = 0;
    for _ in 0..point.sub_batches {
        let block_end = (block_start + per_block).min(records.len());
        let started = Instant::now();
        block_witness(
            &map,
            &keys[block_start..block_end],
            &mut records[block_start..block_end],
            &current,
            &state.payloads,
            value_size,
            &mut olds,
            &mut engine,
        )?;
        witness_ms += started.elapsed().as_secs_f64() * 1e3;

        let started = Instant::now();
        for (key, record) in keys[block_start..block_end]
            .iter()
            .zip(&records[block_start..block_end])
        {
            apply_record(&mut map, &mut current, &state.payloads, *key, record)?;
        }
        apply_ms += started.elapsed().as_secs_f64() * 1e3;
        block_start = block_end;
    }

    let (witness, mut sidecar) = assemble(
        header(point, &state.map, records.len() as u32),
        &encode_records(&records, key_size, value_size),
        &olds,
        &engine,
    );
    sidecar.distinct_keys = distinct_keys(&records);
    sidecar.state_build_ms = state.build_ms;
    sidecar.apply_ms = apply_ms;
    sidecar.witness_ms = witness_ms;
    sidecar.expected_new_root = Some(H::export(&map.root()));
    Ok((witness, sidecar))
}

fn distinct_keys(records: &[Rec]) -> u64 {
    let keys: HashSet<&[u8]> = records.iter().map(|record| record.key.as_slice()).collect();
    keys.len() as u64
}

/// Where a record's leaf will be in the guest's entries.
enum Target {
    Old(u64),
    New(usize),
}

/// Writes one block's touched leaves and siblings, and sets each record's hint.
#[allow(clippy::too_many_arguments)]
fn block_witness<H: ImmHasher>(
    map: &IndexedMerkleMap<H>,
    keys: &[Digest],
    records: &mut [Rec],
    current: &HashMap<Digest, Vec<u8>>,
    base: &HashMap<Digest, Vec<u8>>,
    value_size: usize,
    olds: &mut Vec<u8>,
    engine: &mut Vec<u8>,
) -> Result<()> {
    let n_old = map.length();
    let mut new_keys: BTreeMap<Digest, usize> = BTreeMap::new();
    let mut touched_set = BTreeSet::new();
    let mut targets = Vec::with_capacity(records.len());
    let mut first_updates: HashMap<Digest, ()> = HashMap::new();

    for (key, record) in keys.iter().zip(records.iter()) {
        let target = match record.op {
            OP_INSERT => {
                if map.index_of(key).is_some() || new_keys.contains_key(key) {
                    bail!("insert of an existing key");
                }
                let old_low = map.low_index(key);
                let new_low = new_keys.range(..*key).next_back();
                let target = match new_low {
                    Some((new_key, ordinal)) if *new_key > map.leaf(old_low).key => {
                        Target::New(*ordinal)
                    }
                    _ => {
                        touched_set.insert(old_low);
                        Target::Old(old_low)
                    }
                };
                new_keys.insert(*key, new_keys.len());
                target
            }
            OP_UPDATE => match new_keys.get(key) {
                Some(ordinal) => Target::New(*ordinal),
                None => {
                    let index = map
                        .index_of(key)
                        .ok_or_else(|| anyhow!("update of an absent key"))?;
                    touched_set.insert(index);
                    if first_updates.insert(*key, ()).is_none() {
                        let old = current
                            .get(key)
                            .or_else(|| base.get(key))
                            .ok_or_else(|| anyhow!("no payload for a key"))?;
                        pad_extend(olds, old, value_size);
                    }
                    Target::Old(index)
                }
            },
            op => bail!("bad operation {op}"),
        };
        targets.push(target);
    }

    let touched: Vec<u64> = touched_set.into_iter().collect();
    for (record, target) in records.iter_mut().zip(&targets) {
        record.hint = match target {
            Target::Old(index) => touched.binary_search(index).expect("touched") as u64,
            Target::New(ordinal) => (touched.len() + ordinal) as u64,
        };
    }

    let n_new = n_old + new_keys.len() as u64;
    let mut siblings = Vec::new();
    collect_siblings(
        map,
        n_old,
        n_new,
        &touched,
        map.depth(),
        0,
        0,
        touched.len(),
        &mut siblings,
    );

    let block = BlockHeader {
        records: records.len() as u64,
        touched: touched.len() as u64,
        siblings: siblings.len() as u64,
    };
    extend_raw(engine, core::slice::from_ref(&block));
    let leaves: Vec<TouchedLeaf> = touched
        .iter()
        .map(|index| TouchedLeaf {
            index: *index,
            leaf: *map.leaf(*index),
        })
        .collect();
    extend_raw(engine, &leaves);
    extend_raw(engine, &siblings);
    Ok(())
}

/// The siblings a guest's walk reads, in the order it reads them.
#[allow(clippy::too_many_arguments)]
fn collect_siblings<H: ImmHasher>(
    map: &IndexedMerkleMap<H>,
    n_old: u64,
    n_new: u64,
    touched: &[u64],
    level: usize,
    index: u64,
    lo: usize,
    hi: usize,
    out: &mut Vec<Digest>,
) {
    let start = index << level;
    let end = start + (1u64 << level);
    if lo == hi && start < n_old && (end <= n_old || n_new == n_old) {
        out.push(map.node(level, index));
        return;
    }
    if start >= n_new || level == 0 {
        return;
    }
    let boundary = start + (1u64 << (level - 1));
    let mid = lo + touched[lo..hi].partition_point(|leaf| *leaf < boundary);
    collect_siblings(
        map,
        n_old,
        n_new,
        touched,
        level - 1,
        2 * index,
        lo,
        mid,
        out,
    );
    collect_siblings(
        map,
        n_old,
        n_new,
        touched,
        level - 1,
        2 * index + 1,
        mid,
        hi,
        out,
    );
}

/// Appends plain, padding-free values as their bytes.
fn extend_raw<T: Copy>(bytes: &mut Vec<u8>, values: &[T]) {
    // SAFETY: every `T` here is `repr(C)` integers and digests with no padding.
    bytes.extend_from_slice(unsafe {
        core::slice::from_raw_parts(values.as_ptr().cast::<u8>(), size_of_val(values))
    });
}

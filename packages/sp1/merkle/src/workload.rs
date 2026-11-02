//! The records of a batch. All of it is deterministic in the seed, so a witness can be rebuilt.
//!
//! The record shape and the `mixed` proportions are assumptions, because no target workload exists.

use std::collections::HashSet;

use merkle_map::record::{OP_INSERT, OP_UPDATE, padded};
use rand::{RngExt, SeedableRng, rngs::ChaCha20Rng, seq::SliceRandom};

use crate::point::{Point, Workload};

/// The share of updates in `mixed`.
const MIXED_UPDATE_SHARE: f64 = 0.8;

/// One record of a batch, before it goes into a witness.
#[derive(Clone, Debug)]
pub struct Rec {
    pub op: u8,
    pub hint: u64,
    pub key: Vec<u8>,
    pub arg: Vec<u8>,
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn fill(seed: u64, domain: u64, index: u64, out: &mut [u8]) {
    let base = splitmix(seed ^ splitmix(domain) ^ splitmix(index.wrapping_mul(0x1000_0000_01b3)));
    for (position, chunk) in out.chunks_mut(8).enumerate() {
        let word = splitmix(base.wrapping_add(position as u64)).to_le_bytes();
        chunk.copy_from_slice(&word[..chunk.len()]);
    }
}

/// The preimage of record `index`'s key. An 8-byte preimage is a sequence number starting at 1.
pub fn key_preimage(seed: u64, index: u64, size: usize) -> Vec<u8> {
    if size == 8 {
        return (index + 1).to_le_bytes().to_vec();
    }
    let mut key = vec![0u8; size];
    fill(seed, 1, index, &mut key);
    key
}

/// Record `index`'s starting payload: an amount, a zero nonce, then opaque bytes.
pub fn initial_payload(seed: u64, index: u64, size: usize) -> Vec<u8> {
    let mut payload = vec![0u8; size];
    fill(seed, 2, index, &mut payload);
    payload[8..24].fill(0);
    payload
}

fn update_arg(rng: &mut ChaCha20Rng, size: usize) -> Vec<u8> {
    let mut arg = vec![0u8; size];
    let delta: u64 = rng.random_range(1..1 << 40);
    arg[..8].copy_from_slice(&delta.to_le_bytes());
    arg
}

/// A Zipf rank with exponent 1, by inverting the continuous distribution, moved to a fixed
/// pseudo-random record so hot keys are spread over the key space.
fn zipf_index(rng: &mut ChaCha20Rng, seed: u64, state: u64) -> u64 {
    let unit: f64 = rng.random();
    let rank = ((state as f64 + 1.0).powf(unit) as u64).saturating_sub(1);
    splitmix(seed ^ rank) % state
}

pub fn records(point: &Point) -> Vec<Rec> {
    records_of_block(point, 0)
}

/// The records of one block of a chain. Each block makes its own choices of keys and changes.
pub fn records_of_block(point: &Point, block: u64) -> Vec<Rec> {
    let (seed, state, k) = (point.seed, point.state, point.k as u64);
    let (key_size, value_size) = (point.key_size as usize, point.value_size as usize);
    let block_seed = block.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let mut rng =
        ChaCha20Rng::seed_from_u64(seed ^ splitmix(k) ^ point.workload as u64 ^ block_seed);
    let insert = |index: u64| Rec {
        op: OP_INSERT,
        hint: 0,
        key: key_preimage(seed, index, key_size),
        arg: initial_payload(seed, index, value_size),
    };
    match point.workload {
        Workload::Insert | Workload::Append => (state..state + k).map(insert).collect(),
        Workload::Update => {
            assert!(k <= state, "more updates than records");
            let mut chosen = HashSet::new();
            while (chosen.len() as u64) < k {
                chosen.insert(rng.random_range(0..state));
            }
            // A set's order differs per run, so sort first. Records then arrive in random order, as
            // they would for real: sorted records would share tree paths across sub-batches.
            let mut chosen: Vec<u64> = chosen.into_iter().collect();
            chosen.sort_unstable();
            chosen.shuffle(&mut rng);
            chosen
                .into_iter()
                .map(|index| Rec {
                    op: OP_UPDATE,
                    hint: 0,
                    key: key_preimage(seed, index, key_size),
                    arg: update_arg(&mut rng, value_size),
                })
                .collect()
        }
        Workload::Mixed => {
            let mut next_new = state;
            (0..k)
                .map(|_| {
                    if rng.random::<f64>() < MIXED_UPDATE_SHARE {
                        let index = zipf_index(&mut rng, seed, state);
                        Rec {
                            op: OP_UPDATE,
                            hint: 0,
                            key: key_preimage(seed, index, key_size),
                            arg: update_arg(&mut rng, value_size),
                        }
                    } else {
                        next_new += 1;
                        insert(next_new - 1)
                    }
                })
                .collect()
        }
    }
}

/// Lays out records as a witness's record region.
pub fn encode_records(records: &[Rec], key_size: usize, value_size: usize) -> Vec<u8> {
    let stride = 8 + padded(key_size) + padded(value_size);
    let mut bytes = Vec::with_capacity(records.len() * stride);
    for record in records {
        bytes.extend_from_slice(&((record.op as u64) | (record.hint << 8)).to_le_bytes());
        pad_extend(&mut bytes, &record.key, key_size);
        pad_extend(&mut bytes, &record.arg, value_size);
    }
    bytes
}

pub fn pad_extend(bytes: &mut Vec<u8>, data: &[u8], size: usize) {
    assert_eq!(data.len(), size);
    bytes.extend_from_slice(data);
    bytes.resize(bytes.len() + padded(size) - size, 0);
}

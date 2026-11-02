//! One thing to measure, and where its files live.

use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::arm::Arm;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Workload {
    /// New records with hashed keys.
    Insert,
    /// Changes to existing records, spread uniformly, each key once.
    Update,
    /// 80% changes with Zipf hot keys, so some repeat, and 20% new records.
    Mixed,
    /// New records with sequence-number keys. Jellyfish hashes them, so there it is a short insert.
    Append,
}

impl Workload {
    pub fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Mixed => "mixed",
            Self::Append => "append",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Counts and checks only, with no proof.
    Execute,
    Core,
    Compressed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub arm: Arm,
    pub workload: Workload,
    pub k: u32,
    pub sub_batches: u32,
    pub state: u64,
    pub height: u32,
    pub key_size: u32,
    pub value_size: u32,
    pub read_proofs: bool,
    pub external_batch_hash: bool,
    pub stop_after: u32,
    pub mode: RunMode,
    pub seed: u64,
}

impl Point {
    /// Names the witness, which the stop point and proof mode do not change.
    pub fn witness_id(&self) -> String {
        format!(
            "{}-{}-k{}-b{}-n{}-h{}-key{}-val{}-r{}-x{}-s{}",
            self.arm.name(),
            serde_json::to_value(self.workload)
                .unwrap()
                .as_str()
                .unwrap(),
            self.k,
            self.sub_batches,
            self.state,
            self.height,
            self.key_size,
            self.value_size,
            self.read_proofs as u8,
            self.external_batch_hash as u8,
            self.seed
        )
    }

    /// Identifies a measurement among the recorded runs.
    pub fn run_id(&self) -> String {
        format!(
            "{}-stop{}-{}",
            self.witness_id(),
            self.stop_after,
            serde_json::to_value(self.mode).unwrap().as_str().unwrap()
        )
    }

    pub fn witness_path(&self) -> PathBuf {
        PathBuf::from("data/witness").join(format!("{}.bin", self.witness_id()))
    }

    pub fn sidecar_path(&self) -> PathBuf {
        PathBuf::from("data/witness").join(format!("{}.json", self.witness_id()))
    }
}

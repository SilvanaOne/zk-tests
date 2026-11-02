//! Consecutive blocks on one growing state, as a chain would run them: each block's guest has to start
//! from the root the last one committed, and on the indexed map from its length.
//!
//! This only executes the guests. A proof of one of these blocks costs what the proof rounds measure
//! for one batch, because proofs of consecutive blocks share nothing.

use std::{collections::BTreeMap, fs, io::Write, marker::PhantomData};

use anyhow::{Result, anyhow, ensure};
use jmt::SimpleHasher;
use merkle_map::{
    hash::{Hasher, ImmHasher, JmtHasher, Poseidon2, Sha256Raw},
    jellyfish::hashers::PoseidonNode,
    record::OP_INSERT,
};
use serde::{Deserialize, Serialize};
use sp1_sdk::{LightProver, Prover, SP1Stdin};

use crate::{
    arm::Arm,
    imm_state::{self, ImmState},
    jmt_state::{self, State},
    measure,
    point::{Point, RunMode, Workload},
    prepare::Sidecar,
    software::{Native, SyscallCounts},
    workload::{self, Rec},
};

pub const CONSECUTIVE_PATH: &str = "results/consecutive.jsonl";
/// The height a state of tens of millions of records would need.
const HEIGHT: u32 = 26;

/// One block of a chain, as one line of the results file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub arm: String,
    pub workload: String,
    pub block: u64,
    pub state_before: u64,
    pub records: u64,
    pub distinct_keys: u64,
    pub gas: u64,
    pub cycles: u64,
    pub calls: SyscallCounts,
    pub witness_bytes: u64,
    pub old_len: u64,
    pub new_len: u64,
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub elf_sha256: String,
}

/// A map that a chain of blocks runs on.
trait Chain {
    fn witness(&self, point: &Point, records: &[Rec]) -> Result<(Vec<u8>, Sidecar)>;
    fn apply(&mut self, point: &Point, records: &[Rec]) -> Result<()>;
    fn root(&self) -> [u8; 32];
}

impl<H: ImmHasher> Chain for ImmState<H> {
    fn witness(&self, point: &Point, records: &[Rec]) -> Result<(Vec<u8>, Sidecar)> {
        imm_state::batch_witness_of(point, self, records.to_vec())
    }
    fn apply(&mut self, point: &Point, records: &[Rec]) -> Result<()> {
        ImmState::apply(self, point, records)
    }
    fn root(&self) -> [u8; 32] {
        H::export(&self.map.root())
    }
}

struct Jellyfish<H, S>(State<S>, PhantomData<H>);

impl<H: Hasher + JmtHasher, S: SimpleHasher> Chain for Jellyfish<H, S> {
    fn witness(&self, point: &Point, records: &[Rec]) -> Result<(Vec<u8>, Sidecar)> {
        jmt_state::batch_witness_of::<H, S>(point, &self.0, records)
    }
    fn apply(&mut self, _point: &Point, records: &[Rec]) -> Result<()> {
        self.0.apply(records)
    }
    fn root(&self) -> [u8; 32] {
        self.0.root.0
    }
}

/// The `consecutive` subcommand: on each batch arm, `blocks` blocks of new records, then `blocks`
/// blocks of the mixed workload, all on one state.
pub async fn run(blocks: u64, k: u32, state: u64) -> Result<()> {
    fs::create_dir_all("results")?;
    // A run that fails leaves the last complete results in place.
    let temporary = format!("{CONSECUTIVE_PATH}.tmp");
    let mut file = fs::File::create(&temporary)?;
    let light = LightProver::new().await;
    let mut all = Vec::new();
    for arm in [
        Arm::ImmBatchSha256,
        Arm::ImmBatchPoseidon2,
        Arm::JmtBatchSha256,
        Arm::JmtBatchPoseidon2,
    ] {
        let base = Point {
            arm,
            workload: Workload::Insert,
            k,
            sub_batches: 1,
            state,
            height: if arm.is_jmt() { 0 } else { HEIGHT },
            key_size: 84,
            value_size: 24,
            read_proofs: false,
            external_batch_hash: false,
            stop_after: 0,
            mode: RunMode::Execute,
            seed: 1,
        };
        println!("{}: building {state} records", arm.name());
        let rows = match arm {
            Arm::ImmBatchSha256 => {
                let mut map = ImmState::<Sha256Raw<Native>>::build(&base);
                chain(&light, &base, blocks, &mut map).await?
            }
            Arm::ImmBatchPoseidon2 => {
                let mut map = ImmState::<Poseidon2<Native>>::build(&base);
                chain(&light, &base, blocks, &mut map).await?
            }
            Arm::JmtBatchSha256 => {
                let tree = State::<sha2::Sha256>::build(1, state, 84, 24)?;
                let mut map = Jellyfish::<Sha256Raw<Native>, _>(tree, PhantomData);
                chain(&light, &base, blocks, &mut map).await?
            }
            Arm::JmtBatchPoseidon2 => {
                let tree = State::<PoseidonNode<Native>>::build(1, state, 84, 24)?;
                let mut map = Jellyfish::<Poseidon2<Native>, _>(tree, PhantomData);
                chain(&light, &base, blocks, &mut map).await?
            }
            _ => unreachable!("only batch arms run consecutive blocks"),
        };
        for row in &rows {
            writeln!(file, "{}", serde_json::to_string(row)?)?;
        }
        file.flush()?;
        all.extend(rows);
    }
    fs::rename(temporary, CONSECUTIVE_PATH)?;
    print_summary(&all);
    Ok(())
}

async fn chain(
    light: &LightProver,
    base: &Point,
    blocks: u64,
    map: &mut impl Chain,
) -> Result<Vec<Block>> {
    let arm = base.arm;
    let elf = arm.elf();
    let elf_sha256 = measure::sha256_hex(&elf);
    let mut rows = Vec::new();
    let mut size = base.state;
    let mut root = map.root();
    for (block, workload) in (0..blocks)
        .map(|_| Workload::Insert)
        .chain((0..blocks).map(|_| Workload::Mixed))
        .enumerate()
    {
        let block = block as u64;
        let point = Point {
            workload,
            state: size,
            ..base.clone()
        };
        let records = workload::records_of_block(&point, block);
        let (witness, sidecar) = map.witness(&point, &records)?;
        let words = measure::aligned(&witness);
        let witness = measure::as_bytes(&words);

        let (expected, calls) = measure::expected_run(arm, witness);
        let mut stdin = SP1Stdin::new();
        stdin.write_slice(witness);
        let (public_values, report) = light
            .execute(elf.clone(), stdin)
            .await
            .map_err(|error| anyhow!("{error}"))?;
        measure::check_execution(&report, &public_values, &expected, calls)?;

        ensure!(expected.old_root == root, "block {block} starts elsewhere");
        // The indexed map counts its zero leaf. Jellyfish arms commit no length.
        let length = if arm.is_jmt() { 0 } else { size + 1 };
        ensure!(
            expected.old_len == length,
            "block {block} starts at another length"
        );
        ensure!(
            sidecar.expected_new_root == Some(expected.new_root),
            "block {block} ends at another root than the host's"
        );
        map.apply(&point, &records)?;
        ensure!(
            map.root() == expected.new_root,
            "block {block} ends at another root than the applied state's"
        );
        root = expected.new_root;

        let gas = report.gas().unwrap_or_default();
        println!(
            "{} block {block} {}: {size} records before, {:.0} gas per record",
            arm.name(),
            workload.name(),
            gas as f64 / base.k as f64
        );
        rows.push(Block {
            arm: arm.name().to_string(),
            workload: workload.name().to_string(),
            block,
            state_before: size,
            records: records.len() as u64,
            distinct_keys: sidecar.distinct_keys,
            gas,
            cycles: report.total_instruction_count(),
            calls,
            witness_bytes: sidecar.witness_bytes,
            old_len: expected.old_len,
            new_len: expected.new_len,
            old_root: expected.old_root,
            new_root: expected.new_root,
            elf_sha256: elf_sha256.clone(),
        });
        size += records
            .iter()
            .filter(|record| record.op == OP_INSERT)
            .count() as u64;
    }
    Ok(rows)
}

fn print_summary(rows: &[Block]) {
    let mut groups: BTreeMap<(&str, &str), Vec<f64>> = BTreeMap::new();
    for row in rows {
        groups
            .entry((&row.arm, &row.workload))
            .or_default()
            .push(row.gas as f64 / row.records as f64);
    }
    println!(
        "gas per record over consecutive blocks; every block started from the last one's root"
    );
    for ((arm, workload), gas) in groups {
        let mean = gas.iter().sum::<f64>() / gas.len() as f64;
        let low = gas.iter().copied().fold(f64::MAX, f64::min);
        let high = gas.iter().copied().fold(0.0, f64::max);
        println!(
            "{arm:22} {workload:7} {} blocks: mean {mean:.0}, min {low:.0}, max {high:.0}",
            gas.len()
        );
    }
}

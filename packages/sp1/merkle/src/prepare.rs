//! Builds trees and writes witnesses, one process per tree, so a tree's memory is gone before any
//! proof starts.

use std::{collections::HashMap, fs, path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use jmt::SimpleHasher;
use merkle_map::{
    hash::{Hasher, ImmHasher, JmtHasher, Poseidon2, Sha256Raw},
    jellyfish::hashers::{PoseidonNode, PoseidonSponge},
    record::{ABI_VERSION, HEADER_BYTES, Header},
};
use serde::{Deserialize, Serialize};

use crate::{
    arm::{Engine, Group},
    imm_state::{self, ImmState},
    jmt_state::{FrameStats, State, batch_witness, stock_witness},
    point::Point,
    software::Native,
};

/// What a witness's generation measured.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sidecar {
    pub witness_bytes: u64,
    pub records_bytes: u64,
    pub olds_bytes: u64,
    pub engine_bytes: u64,
    pub distinct_keys: u64,
    pub state_build_ms: f64,
    /// Applying the batch natively without proofs, as a validator replaying it would.
    pub apply_ms: f64,
    pub witness_ms: f64,
    pub frames: Option<FrameStats>,
    /// The host's own new root, which the engine's output must equal.
    pub expected_new_root: Option<[u8; 32]>,
    pub tags: Option<u64>,
    pub digests: Option<u64>,
}

/// Lays out a witness: header, records, old payloads, engine region.
pub fn assemble(
    mut header: Header,
    records: &[u8],
    olds: &[u8],
    engine: &[u8],
) -> (Vec<u8>, Sidecar) {
    header.abi_version = ABI_VERSION;
    header.records_bytes = records.len() as u64;
    header.olds_bytes = olds.len() as u64;
    header.engine_bytes = engine.len() as u64;
    let mut bytes = Vec::with_capacity(HEADER_BYTES + records.len() + olds.len() + engine.len());
    // SAFETY: the header is `repr(C)` with no padding.
    bytes.extend_from_slice(unsafe {
        core::slice::from_raw_parts((&raw const header).cast::<u8>(), HEADER_BYTES)
    });
    bytes.extend_from_slice(records);
    bytes.extend_from_slice(olds);
    bytes.extend_from_slice(engine);
    let sidecar = Sidecar {
        witness_bytes: bytes.len() as u64,
        records_bytes: records.len() as u64,
        olds_bytes: olds.len() as u64,
        engine_bytes: engine.len() as u64,
        ..Default::default()
    };
    (bytes, sidecar)
}

/// What makes two points share a built tree: the group, state size, height, key and value sizes,
/// and seed.
type TreeKey = (Group, u64, u32, u32, u32, u64);

/// Writes any missing witnesses, one child process per tree.
pub fn ensure_witnesses(points: &[Point]) -> Result<()> {
    let mut groups: HashMap<TreeKey, Vec<&Point>> = HashMap::new();
    for point in points.iter().filter(|point| !point.witness_path().exists()) {
        groups
            .entry((
                point.arm.group(),
                point.state,
                point.height,
                point.key_size,
                point.value_size,
                point.seed,
            ))
            .or_default()
            .push(point);
    }
    fs::create_dir_all("data/witness")?;
    for ((group, state, ..), points) in groups {
        println!(
            "Building {group:?} with {state} records for {} witnesses...",
            points.len()
        );
        let list = format!("data/witness/prepare-{}.json", std::process::id());
        fs::write(&list, serde_json::to_vec(&points)?)?;
        let status = Command::new(std::env::current_exe()?)
            .args(["prepare", "--points", &list])
            .status()?;
        fs::remove_file(&list).ok();
        ensure!(status.success(), "preparing witnesses failed: {status}");
    }
    Ok(())
}

/// The `prepare` subcommand: builds one tree, then writes the witnesses of the listed points.
pub fn run(list: &Path) -> Result<()> {
    let points: Vec<Point> = serde_json::from_slice(&fs::read(list)?)?;
    let first = points.first().context("no points")?;
    match first.arm.group() {
        Group::JmtSha256 => jmt_group::<Sha256Raw<Native>, sha2::Sha256>(&points),
        Group::JmtPoseidon2Sponge => {
            jmt_group::<Poseidon2<Native>, PoseidonSponge<Native>>(&points)
        }
        Group::JmtPoseidon2Node => jmt_group::<Poseidon2<Native>, PoseidonNode<Native>>(&points),
        Group::ImmSha256 => imm_group::<Sha256Raw<Native>>(&points),
        Group::ImmPoseidon2 => imm_group::<Poseidon2<Native>>(&points),
    }
}

fn imm_group<H: ImmHasher>(points: &[Point]) -> Result<()> {
    let state = ImmState::<H>::build(&points[0]);
    println!(
        "  {} records in {:.1} s, {:.1} us per record",
        points[0].state,
        state.build_ms / 1e3,
        state.build_ms * 1e3 / points[0].state.max(1) as f64
    );
    for point in points {
        let (witness, sidecar) = match point.arm.engine() {
            Engine::Chained => imm_state::chained_witness(point, &state)?,
            Engine::Batch => imm_state::batch_witness(point, &state)?,
            Engine::Stock => unreachable!("indexed map arms have no stock engine"),
        };
        save(point, &witness, &sidecar)?;
    }
    Ok(())
}

fn jmt_group<H: Hasher + JmtHasher, S: SimpleHasher>(points: &[Point]) -> Result<()> {
    let first = &points[0];
    let state = State::<S>::build(
        first.seed,
        first.state,
        first.key_size as usize,
        first.value_size as usize,
    )?;
    println!(
        "  {} records in {:.1} s, {:.1} us per record",
        first.state,
        state.build_ms / 1e3,
        state.build_ms * 1e3 / first.state.max(1) as f64
    );
    for point in points {
        let (witness, sidecar) = match point.arm.engine() {
            Engine::Batch => batch_witness::<H, S>(point, &state)?,
            _ => stock_witness::<H, S>(point, &state)?,
        };
        save(point, &witness, &sidecar)?;
    }
    Ok(())
}

/// Builds one point's state and witness, for tests of a guest on a small tree.
pub fn witness_for(point: &Point) -> Result<(Vec<u8>, Sidecar)> {
    fn imm<H: ImmHasher>(point: &Point) -> Result<(Vec<u8>, Sidecar)> {
        let state = ImmState::<H>::build(point);
        match point.arm.engine() {
            Engine::Chained => imm_state::chained_witness(point, &state),
            _ => imm_state::batch_witness(point, &state),
        }
    }
    fn jmt<H: Hasher + JmtHasher, S: SimpleHasher>(point: &Point) -> Result<(Vec<u8>, Sidecar)> {
        let state = State::<S>::build(
            point.seed,
            point.state,
            point.key_size as usize,
            point.value_size as usize,
        )?;
        match point.arm.engine() {
            Engine::Batch => batch_witness::<H, S>(point, &state),
            _ => stock_witness::<H, S>(point, &state),
        }
    }
    match point.arm.group() {
        Group::ImmSha256 => imm::<Sha256Raw<Native>>(point),
        Group::ImmPoseidon2 => imm::<Poseidon2<Native>>(point),
        Group::JmtSha256 => jmt::<Sha256Raw<Native>, sha2::Sha256>(point),
        Group::JmtPoseidon2Sponge => jmt::<Poseidon2<Native>, PoseidonSponge<Native>>(point),
        Group::JmtPoseidon2Node => jmt::<Poseidon2<Native>, PoseidonNode<Native>>(point),
    }
}

fn save(point: &Point, witness: &[u8], sidecar: &Sidecar) -> Result<()> {
    // The engine must reach the host's own root before a witness is worth keeping.
    let words = crate::measure::aligned(witness);
    let output = point.arm.native(crate::measure::as_bytes(&words));
    if let Some(expected) = sidecar.expected_new_root {
        ensure!(
            output.new_root == expected,
            "{}: the engine's new root differs from the host's",
            point.witness_id()
        );
    }
    let temporary = point.witness_path().with_extension("tmp");
    fs::write(&temporary, witness)?;
    fs::write(point.sidecar_path(), serde_json::to_vec_pretty(sidecar)?)?;
    fs::rename(temporary, point.witness_path())?;
    Ok(())
}

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use merkle_bench::{
    arm::Arm,
    consecutive, measure,
    point::{Point, RunMode, Workload},
    prepare, report, rounds,
};

#[derive(Parser)]
#[command(
    version,
    about = "Adds and changes many records through real Merkle maps in SP1"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Measures one point: prepares its witness if needed, executes it, and proves it if asked.
    One {
        #[arg(long, value_enum)]
        arm: Arm,
        #[arg(long, value_enum, default_value = "insert")]
        workload: Workload,
        /// Records in the batch.
        #[arg(short, long, default_value_t = 100)]
        k: u32,
        /// Records already in the state.
        #[arg(long, default_value_t = 1_000_000)]
        state: u64,
        #[arg(long, value_enum, default_value = "execute")]
        mode: RunMode,
        /// Indexed map height.
        #[arg(long, default_value_t = 26)]
        height: u32,
        #[arg(long, default_value_t = 84)]
        key_size: u32,
        #[arg(long, default_value_t = 24)]
        value_size: u32,
        #[arg(long)]
        read_proofs: bool,
        #[arg(long)]
        external_batch_hash: bool,
        #[arg(long, default_value_t = 0)]
        stop_after: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Measures a named list of points, skipping any already recorded.
    Run {
        /// execute, deep, r1, r1s, r2a, r2b, r3, r3s, r4, r5 or compressed.
        round: String,
    },
    /// Executes consecutive blocks on one growing state, on each batch arm, chaining their roots.
    Consecutive {
        /// Blocks of new records, and then as many of the mixed workload.
        #[arg(long, default_value_t = 20)]
        blocks: u64,
        /// Records per block.
        #[arg(short, long, default_value_t = 1_000)]
        k: u32,
        /// Records in the state before the first block.
        #[arg(long, default_value_t = 1_000_000)]
        state: u64,
    },
    /// Internal: builds one tree and writes the witnesses of the listed points.
    Prepare {
        #[arg(long)]
        points: PathBuf,
    },
    /// Internal: measures one point in its own process.
    Measure {
        #[arg(long)]
        point: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Prints tables from the recorded runs.
    Report,
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::One {
            arm,
            workload,
            k,
            state,
            mode,
            height,
            key_size,
            value_size,
            read_proofs,
            external_batch_hash,
            stop_after,
            seed,
        } => {
            let point = Point {
                arm,
                workload,
                k,
                sub_batches: 1,
                state,
                height: if arm.is_jmt() { 0 } else { height },
                // A sequence number is the key's preimage.
                key_size: if workload == Workload::Append {
                    8
                } else {
                    key_size
                },
                value_size,
                read_proofs,
                external_batch_hash,
                stop_after,
                mode,
                seed,
            };
            prepare::ensure_witnesses(std::slice::from_ref(&point))?;
            measure::run_point(&point)?;
        }
        Command::Run { round } => {
            let points = rounds::round(&round).context("unknown round")?;
            let missing: Vec<Point> = points
                .iter()
                .filter(|point| !measure::is_recorded(point))
                .cloned()
                .collect();
            println!(
                "{round}: {} of {} points to run",
                missing.len(),
                points.len()
            );
            prepare::ensure_witnesses(&missing)?;
            for point in &missing {
                measure::run_point(point)?;
            }
            report::print()?;
        }
        Command::Consecutive { blocks, k, state } => consecutive::run(blocks, k, state).await?,
        Command::Prepare { points } => prepare::run(&points)?,
        Command::Measure { point, output } => measure::child(&point, &output).await?,
        Command::Report => report::print()?,
    }
    Ok(())
}

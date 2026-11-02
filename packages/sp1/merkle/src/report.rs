//! Tables from the recorded runs.

use anyhow::Result;
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use merkle_map::record::{STOP_AFTER_DECODE, STOP_AFTER_DERIVE};

use crate::{
    measure::{Run, current_runs},
    point::{Point, Workload},
};

/// Which part of the machine a chip belongs to.
fn chip_group(chip: &str) -> usize {
    if ["Sha", "Poseidon", "Keccak"]
        .iter()
        .any(|hash| chip.contains(hash))
    {
        0
    } else if chip.contains("Memory") || chip.contains("PageProt") || chip == "Global" {
        1
    } else if ["Byte", "Range", "Program"].contains(&chip) {
        3
    } else {
        2
    }
}

/// What sets a run apart from the standard one of its arm, workload and batch size.
fn settings(run: &Run, point: &Point) -> String {
    let mut parts = Vec::new();
    if let Some(workers) = run.env.get("SP1_WORKER_NUM_CORE_WORKERS") {
        parts.push(format!("workers={workers}"));
    }
    match point.stop_after {
        STOP_AFTER_DERIVE => parts.push("stop=derive".to_string()),
        STOP_AFTER_DECODE => parts.push("stop=decode".to_string()),
        _ => {}
    }
    if point.sub_batches != 1 {
        parts.push(format!("blocks={}", point.sub_batches));
    }
    if point.key_size != 84 && point.workload != Workload::Append {
        parts.push(format!("key={}", point.key_size));
    }
    if point.value_size != 24 {
        parts.push(format!("value={}", point.value_size));
    }
    if point.read_proofs {
        parts.push("read proofs".to_string());
    }
    if point.external_batch_hash {
        parts.push("external hash".to_string());
    }
    if point.seed != 1 {
        parts.push(format!("seed={}", point.seed));
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" ")
    }
}

fn gib(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1u64 << 30) as f64)
}

pub fn print() -> Result<()> {
    let runs = current_runs()?;
    let mut table = Table::new();
    table.load_style(UTF8_FULL_CONDENSED);
    table.set_header([
        "arm",
        "workload",
        "k",
        "state",
        "h",
        "settings",
        "mode",
        "status",
        "gas/rec",
        "instr/rec",
        "sha c/rec",
        "p2/rec",
        "wit B/rec",
        "native ms",
        "prove s",
        "shards",
        "M cells",
        "cells/rec",
        "us/gas",
        "RSS GiB",
        "peak GiB",
        "swap+ GiB",
        "other cores",
    ]);
    for run in &runs {
        let Some(point) = &run.point else { continue };
        let k = point.k.max(1) as f64;
        let per = |value: f64| format!("{:.0}", value / k);
        // A run that failed before it finished executing has nothing to show.
        let executed = run.gas.is_some();
        let or_dash = |text: String| if executed { text } else { "-".to_string() };
        let counts = run.counts.unwrap_or_default();
        let sidecar = run.sidecar.clone().unwrap_or_default();
        table.add_row(vec![
            point.arm.name().to_string(),
            serde_json::to_value(point.workload)?
                .as_str()
                .unwrap_or("")
                .to_string(),
            point.k.to_string(),
            point.state.to_string(),
            point.height.to_string(),
            settings(run, point),
            serde_json::to_value(point.mode)?
                .as_str()
                .unwrap_or("")
                .to_string(),
            match run.status.as_str() {
                "stopped" if run.stopped_for_memory() => "stopped: memory".to_string(),
                "stopped" => "stopped: time".to_string(),
                status => status.to_string(),
            },
            or_dash(per(run.gas.unwrap_or_default() as f64)),
            or_dash(per(run.cycles as f64)),
            or_dash(format!("{:.1}", counts.sha_compress as f64 / k)),
            or_dash(format!("{:.1}", counts.poseidon2 as f64 / k)),
            or_dash(per(sidecar.witness_bytes as f64)),
            or_dash(format!("{:.1}", run.native_ms)),
            run.prove_ms
                .map_or("-".to_string(), |ms| format!("{:.1}", ms / 1e3)),
            run.shards
                .map_or("-".to_string(), |shards| shards.to_string()),
            run.trace_area
                .map_or("-".to_string(), |area| format!("{:.1}", area as f64 / 1e6)),
            run.trace_area
                .map_or("-".to_string(), |area| per(area as f64)),
            match (run.prove_ms, run.gas) {
                (Some(ms), Some(gas)) if run.status == "ok" => {
                    format!("{:.1}", ms * 1e3 / gas as f64)
                }
                _ => "-".to_string(),
            },
            run.peak_rss_bytes.map_or("-".to_string(), gib),
            run.peak_footprint_bytes.map_or("-".to_string(), gib),
            run.load
                .map_or("-".to_string(), |load| gib(load.swap_grew_bytes)),
            run.load
                .and_then(|load| load.other_cpu_cores)
                .map_or("-".to_string(), |cores| format!("{cores:.1}")),
        ]);
    }
    println!("{table}");

    let mut chips = Table::new();
    chips.load_style(UTF8_FULL_CONDENSED);
    chips.set_header([
        "arm",
        "workload",
        "k",
        "settings",
        "M cells",
        "hash %",
        "memory %",
        "instructions %",
        "fixed %",
    ]);
    for run in &runs {
        let Some(point) = &run.point else { continue };
        if run.chip_area.is_empty() {
            continue;
        }
        let mut groups = [0u64; 4];
        for (chip, area) in &run.chip_area {
            groups[chip_group(chip)] += area;
        }
        let total: u64 = groups.iter().sum();
        let mut row = vec![
            point.arm.name().to_string(),
            serde_json::to_value(point.workload)?
                .as_str()
                .unwrap_or("")
                .to_string(),
            point.k.to_string(),
            settings(run, point),
            format!("{:.1}", total as f64 / 1e6),
        ];
        row.extend(
            groups
                .iter()
                .map(|area| format!("{:.1}", *area as f64 / total as f64 * 100.0)),
        );
        chips.add_row(row);
    }
    println!("{chips}");
    Ok(())
}

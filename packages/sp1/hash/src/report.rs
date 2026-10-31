//! Results, their analysis, and the console and JSON output.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use serde::Serialize;
use sp1_sdk::SP1_CIRCUIT_VERSION;
use sysinfo::System;

use crate::{
    bench::LoadSample,
    cli::{Config, Mode},
    variant::{SyscallCounts, Tier, Variant},
};

const SP1_VERSION: &str = "6.10.0";
const GUEST_TOOLCHAIN: &str = "succinct-1.96.0-64bit-v3";
/// A proof is flagged when other processes used more than this many cores during it.
const BUSY_CORES: f64 = 1.0;
/// A proof is flagged when swap use grew by more than this during it.
const SWAP_GROWTH_BYTES: u64 = 100 << 20;
/// Prove time is only ranked with at least this many warm proofs.
const MIN_RANKED_SAMPLES: usize = 3;
/// What loading one 32-byte child costs. Both children of a node are fresh input in these runs,
/// while a node in a real tree has one already in memory. From SP1's cost table, each of the 4
/// words costs 542 for its global memory init and finalize (30 + 30 + 2 x 241) and 502 for its
/// first read (20 + 2 x 241).
const FRESH_CHILD_AREA: f64 = 4.0 * (542.0 + 502.0);

/// Environment settings that change proving behaviour. Nothing else is recorded, since other
/// variables could hold keys.
const RECORDED_ENV: [&str; 10] = [
    "SP1_PROVER",
    "ELEMENT_THRESHOLD",
    "HEIGHT_THRESHOLD",
    "SHARD_SIZE",
    "FULL_SIZE_SHARDS",
    "MINIMAL_TRACE_CHUNK_THRESHOLD",
    "GAS_TRACE_CHUNK_THRESHOLD",
    "TRACE_CHUNK_SLOTS",
    "MEMORY_LIMIT",
    "RAYON_NUM_THREADS",
];

/// The active prover, by SP1's own default.
pub fn prover_name() -> String {
    std::env::var("SP1_PROVER").unwrap_or_else(|_| "cpu".to_string())
}

pub fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    match sorted.len() {
        0 => f64::NAN,
        len if len % 2 == 1 => sorted[len / 2],
        len => (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0,
    }
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub sp1_version: &'static str,
    pub circuit_version: &'static str,
    pub host_rustc: &'static str,
    pub guest_toolchain: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub cpu: String,
    pub cores: usize,
    pub memory_gib: u64,
    pub prover: String,
    pub seed: u64,
    pub data_sizes: Vec<usize>,
    pub counts: Vec<usize>,
    pub variants: Vec<Variant>,
    pub modes: Vec<Mode>,
    pub runs: usize,
    pub debug_build: bool,
    /// False while a run is in progress, or if it failed partway.
    pub complete: bool,
    pub unix_time: u64,
    pub env_overrides: BTreeMap<String, String>,
    pub run_order: Vec<&'static str>,
    pub programs: Vec<ProgramInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProgramInfo {
    pub variant: Variant,
    pub elf_bytes: usize,
    pub elf_sha256: String,
    pub vkey: String,
}

#[derive(Debug, Serialize)]
pub struct NativeResult {
    pub variant: Variant,
    pub data_size: usize,
    pub count: usize,
    pub median_ns_per_hash: f64,
    pub min_ns_per_hash: f64,
    pub mb_per_s: f64,
    pub samples_ns_per_hash: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub variant: Variant,
    pub data_size: usize,
    pub count: usize,
    /// RISC-V instructions only. Rows inside precompile chips are not counted.
    pub cycles: u64,
    /// SP1's pricing estimate, not trace area. It leaves out global memory init and finalize for
    /// every touched word, and the memory rows deferred precompiles add, so it understates Keccak.
    pub gas: Option<u64>,
    pub syscalls: SyscallCounts,
    pub opcodes: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct ProofRun {
    pub prove_ms: f64,
    pub load: LoadSample,
}

#[derive(Debug, Serialize)]
pub struct ProofResult {
    pub variant: Variant,
    pub data_size: usize,
    pub count: usize,
    pub mode: Mode,
    pub setup_ms: f64,
    pub runs: Vec<ProofRun>,
    pub proof_bytes: u64,
    pub verify_ms: Vec<f64>,
    /// Whether a wrong key and changed public values were rejected. None for the mock prover.
    pub negative_controls_passed: Option<bool>,
    /// Core proofs only.
    pub shards: Option<usize>,
    /// Core proofs only: height times main width, summed over all chips and shards.
    pub trace_area: Option<u64>,
    pub chip_area: BTreeMap<String, u64>,
}

impl ProofResult {
    /// All runs but the first. In compressed mode the first also compiles recursion programs that
    /// later proofs reuse. Core mode follows the same rule so the two stay comparable.
    fn warm_runs(&self) -> &[ProofRun] {
        &self.runs[1..]
    }

    pub fn summary(&self) -> String {
        let proofs: Vec<_> = self
            .runs
            .iter()
            .map(|run| format!("{:.1} s", run.prove_ms / 1e3))
            .collect();
        format!(
            "setup {:.1} s, prove {}, verify {:.1} ms, proof {:.1} KiB{}",
            self.setup_ms / 1e3,
            proofs.join(", "),
            median(&self.verify_ms),
            self.proof_bytes as f64 / 1024.0,
            self.shards
                .map_or(String::new(), |shards| format!(", {shards} shards"))
        )
    }
}

/// One variant at one point of the sweep, with baseline's runs combined.
#[derive(Debug, Clone, Serialize)]
pub struct ProofRow {
    pub variant: Variant,
    pub data_size: usize,
    pub count: usize,
    pub mode: Mode,
    pub setup_ms: f64,
    pub cold_ms: f64,
    pub warm_median_ms: Option<f64>,
    pub warm_spread_ms: Option<f64>,
    pub warm_samples: usize,
    /// From warm proofs if there are any, otherwise cold ones. Includes any fixed cost of extra
    /// shards, divided by the count.
    pub net_ms_per_hash: Option<f64>,
    pub shards: Option<usize>,
    pub trace_area: Option<u64>,
    pub net_area_per_hash: Option<f64>,
    pub proof_bytes: u64,
    pub verify_median_ms: f64,
    pub flags: Vec<&'static str>,
}

/// `time = fixed + per_hash * count`, fitted across the counts of a sweep.
#[derive(Debug, Clone, Serialize)]
pub struct Slope {
    pub variant: Variant,
    pub data_size: usize,
    pub mode: Mode,
    pub counts: usize,
    pub fixed_ms: f64,
    pub ms_per_hash: f64,
    pub net_ms_per_hash: Option<f64>,
    pub warm_samples: usize,
    pub flags: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub meta: Meta,
    pub native: Vec<NativeResult>,
    pub execution: Vec<ExecutionResult>,
    pub proofs: Vec<ProofResult>,
}

#[derive(Serialize)]
struct ReportFile<'a> {
    #[serde(flatten)]
    report: &'a Report,
    proof_rows: Vec<ProofRow>,
    slopes: Vec<Slope>,
}

impl Report {
    pub fn new(config: &Config) -> Result<Self> {
        let mut system = System::new().context("reading system information")?;
        system.refresh_cpu_all();
        system.refresh_memory();
        let env_overrides = std::env::vars_os()
            .filter_map(|(key, value)| {
                let key = key.into_string().ok()?;
                let recorded =
                    RECORDED_ENV.contains(&key.as_str()) || key.starts_with("SP1_WORKER_");
                recorded.then(|| (key, value.to_string_lossy().into_owned()))
            })
            .collect();
        Ok(Self {
            meta: Meta {
                sp1_version: SP1_VERSION,
                circuit_version: SP1_CIRCUIT_VERSION,
                host_rustc: env!("HOST_RUSTC_VERSION"),
                guest_toolchain: GUEST_TOOLCHAIN,
                os: std::env::consts::OS,
                arch: std::env::consts::ARCH,
                cpu: system
                    .cpus()
                    .first()
                    .map_or_else(String::new, |cpu| cpu.brand().to_string()),
                cores: system.cpus().len(),
                memory_gib: system.total_memory() >> 30,
                prover: prover_name(),
                seed: config.seed,
                data_sizes: config.data_sizes.clone(),
                counts: config.counts.clone(),
                variants: config.variants.clone(),
                modes: config.modes.clone(),
                runs: config.runs,
                debug_build: cfg!(debug_assertions),
                complete: false,
                unix_time: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_secs()),
                env_overrides,
                run_order: Vec::new(),
                programs: Vec::new(),
            },
            native: Vec::new(),
            execution: Vec::new(),
            proofs: Vec::new(),
        })
    }

    pub fn print_header(&self) {
        let meta = &self.meta;
        println!(
            "SP1 {} (circuit {}), prover {}, {} {}, {} cores, {} GiB, seed {}",
            meta.sp1_version,
            meta.circuit_version,
            meta.prover,
            meta.cpu,
            meta.arch,
            meta.cores,
            meta.memory_gib,
            meta.seed
        );
    }

    /// Writes the JSON report, if one was asked for. Called after each measurement, so a failure
    /// partway through a long run keeps what was measured.
    pub fn save(&self, config: &Config) -> Result<()> {
        let Some(path) = &config.json else {
            return Ok(());
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let rows = self.proof_rows();
        let file = ReportFile {
            report: self,
            slopes: slopes(&rows),
            proof_rows: rows,
        };
        fs::write(path, serde_json::to_vec_pretty(&file)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    fn execution_at(
        &self,
        variant: Variant,
        size: usize,
        count: usize,
    ) -> Option<&ExecutionResult> {
        self.execution
            .iter()
            .find(|e| e.variant == variant && e.data_size == size && e.count == count)
    }

    /// Combines the runs of each variant at each point and works out net figures and flags.
    pub fn proof_rows(&self) -> Vec<ProofRow> {
        let mut groups: BTreeMap<(usize, usize, Mode, Variant), Vec<&ProofResult>> =
            BTreeMap::new();
        for proof in &self.proofs {
            groups
                .entry((proof.data_size, proof.count, proof.mode, proof.variant))
                .or_default()
                .push(proof);
        }

        let mut rows = Vec::new();
        for (&(data_size, count, mode, variant), results) in &groups {
            let time = TimeStats::of(results);
            let baseline_results = groups.get(&(data_size, count, mode, Variant::Baseline));
            let baseline = baseline_results.map(|results| TimeStats::of(results));
            // Only core proofs expose shards, so compressed rows take them from the same workload.
            let core_proof = |variant| {
                groups
                    .get(&(data_size, count, Mode::Core, variant))
                    .and_then(|results| results.first())
            };
            let shards = core_proof(variant).and_then(|proof| proof.shards);
            let trace_area = core_proof(variant).and_then(|proof| proof.trace_area);
            let baseline_shards = core_proof(Variant::Baseline).and_then(|proof| proof.shards);
            let baseline_area = core_proof(Variant::Baseline).and_then(|proof| proof.trace_area);

            let mut flags = Vec::new();
            if time.samples == 0 {
                flags.push("cold-only");
            } else if time.samples < MIN_RANKED_SAMPLES {
                flags.push("few-samples");
            }
            // A net figure inherits any disturbance of the baseline it subtracts.
            let runs: Vec<_> = results
                .iter()
                .chain(baseline_results.into_iter().flatten())
                .flat_map(|result| &result.runs)
                .collect();
            if runs.iter().any(|run| {
                run.load
                    .other_cpu_cores
                    .is_some_and(|cores| cores > BUSY_CORES)
            }) {
                flags.push("busy");
            }
            if runs
                .iter()
                .any(|run| run.load.swap_grew_bytes > SWAP_GROWTH_BYTES)
            {
                flags.push("swap");
            }
            if variant != Variant::Baseline {
                if let (Some(own), Some(reference)) = (shards, baseline_shards)
                    && own != reference
                {
                    flags.push("shards");
                }
                if let Some(reference) = &baseline {
                    let delta = (time.basis_ms() - reference.basis_ms()).abs();
                    if time.samples >= MIN_RANKED_SAMPLES
                        && delta < 2.0 * (time.spread_ms() + reference.spread_ms())
                    {
                        flags.push("noise");
                    }
                }
            }

            rows.push(ProofRow {
                variant,
                data_size,
                count,
                mode,
                setup_ms: results.iter().map(|result| result.setup_ms).sum::<f64>()
                    / results.len() as f64,
                cold_ms: time.cold_ms,
                warm_median_ms: time.warm.map(|warm| warm.median_ms),
                warm_spread_ms: time.warm.map(|warm| warm.spread_ms),
                warm_samples: time.samples,
                net_ms_per_hash: (variant != Variant::Baseline)
                    .then(|| {
                        baseline.as_ref().map(|reference| {
                            (time.basis_ms() - reference.basis_ms()) / count as f64
                        })
                    })
                    .flatten(),
                shards,
                trace_area,
                net_area_per_hash: (variant != Variant::Baseline)
                    .then(|| Some((trace_area? as f64 - baseline_area? as f64) / count as f64))
                    .flatten(),
                proof_bytes: results[0].proof_bytes,
                verify_median_ms: median(
                    &results
                        .iter()
                        .flat_map(|result| result.verify_ms.clone())
                        .collect::<Vec<_>>(),
                ),
                flags,
            });
        }
        rows
    }

    pub fn print(&self) {
        let rows = self.proof_rows();
        let slopes = slopes(&rows);
        self.print_native();
        self.print_execution();
        print_proofs(&rows);
        print_slopes(&slopes);
        self.print_rankings(&rows, &slopes);
    }

    fn print_native(&self) {
        let mut table = new_table([
            "variant",
            "bytes",
            "x",
            "ns/hash (median)",
            "ns/hash (min)",
            "MB/s",
        ]);
        for result in &self.native {
            table.add_row([
                result.variant.name().to_string(),
                result.data_size.to_string(),
                result.count.to_string(),
                format!("{:.0}", result.median_ns_per_hash),
                format!("{:.0}", result.min_ns_per_hash),
                format!("{:.0}", result.mb_per_s),
            ]);
        }
        println!(
            "\nNative, single thread. Poseidon2 is the scalar permutation, not batched.\n{table}"
        );
    }

    fn print_execution(&self) {
        let mut table = new_table([
            "variant",
            "bytes",
            "x",
            "instr/hash net",
            "gas/hash net",
            "SHA_EXTEND",
            "SHA_COMPRESS",
            "KECCAK_PERMUTE",
            "POSEIDON2",
        ]);
        for result in &self.execution {
            let (cycles, gas) = self.net_execution(result);
            table.add_row([
                result.variant.name().to_string(),
                result.data_size.to_string(),
                result.count.to_string(),
                format_net(cycles),
                format_net(gas),
                result.syscalls.sha_extend.to_string(),
                result.syscalls.sha_compress.to_string(),
                result.syscalls.keccak_permute.to_string(),
                result.syscalls.poseidon2.to_string(),
            ]);
        }
        println!(
            "\nGuest execution. Net means per hash, minus baseline. Instructions exclude precompile \
             rows. Gas is SP1's pricing estimate, not proving cost.\n{table}"
        );
    }

    /// Cycles and gas per hash, minus baseline.
    fn net_execution(&self, result: &ExecutionResult) -> (Option<f64>, Option<f64>) {
        if result.variant == Variant::Baseline {
            return (None, None);
        }
        let Some(baseline) = self.execution_at(Variant::Baseline, result.data_size, result.count)
        else {
            return (None, None);
        };
        let count = result.count as f64;
        let cycles = (result.cycles as f64 - baseline.cycles as f64) / count;
        let gas = result
            .gas
            .zip(baseline.gas)
            .map(|(own, reference)| (own as f64 - reference as f64) / count);
        (Some(cycles), gas)
    }

    /// Ranks within each tier and data size, never across them.
    fn print_rankings(&self, rows: &[ProofRow], slopes: &[Slope]) {
        let sizes: BTreeSet<_> = self
            .execution
            .iter()
            .map(|result| result.data_size)
            .collect();
        for tier in [Tier::Node, Tier::Bytes] {
            for &size in &sizes {
                let mut lines = Vec::new();

                if let Some(area) = area_ranking(rows, tier, size) {
                    lines.push(ranked(&area.title, area.net, 0));
                    if tier == Tier::Node {
                        lines.push(match area.real_node {
                            Some(real_node) => ranked(
                                "Estimated real tree node: fitted area per hash, minus loading the \
                                 child a real path already has in memory. Compare hashes on this one",
                                real_node,
                                0,
                            ),
                            None => {
                                "  The estimated real tree node needs at least 2 counts in -n".to_string()
                            }
                        });
                    }
                } else if let Some(entries) = self.gas_ranking(tier, size) {
                    lines.push(ranked(
                        "SP1 gas estimate per hash, net of baseline. No core proofs, so no trace \
                         area, and gas can misorder SHA-256 and Keccak",
                        entries,
                        1,
                    ));
                }

                // Mock proofs only execute the guest, so their time says nothing about proving.
                if prover_name() != "mock" {
                    for mode in [Mode::Core, Mode::Compressed] {
                        lines.extend(time_ranking(rows, slopes, tier, size, mode));
                    }
                }

                if !lines.is_empty() {
                    let tier_name = if tier == Tier::Node { "node" } else { "bytes" };
                    println!("\nRanking: {tier_name} tier, {size} bytes");
                    for line in lines {
                        println!("{line}");
                    }
                }
            }
        }
        if rows.iter().any(|row| row.warm_samples < MIN_RANKED_SAMPLES) {
            println!(
                "\nProve time is not ranked below {MIN_RANKED_SAMPLES} warm proofs: use --runs 4 or more."
            );
        }
    }

    /// Net gas per hash at the largest count.
    fn gas_ranking(&self, tier: Tier, size: usize) -> Option<Vec<RankEntry>> {
        let gas: Vec<_> = self
            .execution
            .iter()
            .filter(|result| result.variant.tier() == tier && result.data_size == size)
            .filter_map(|result| Some((result, self.net_execution(result).1?)))
            .collect();
        let largest = gas.iter().map(|(result, _)| result.count).max()?;
        Some(
            gas.iter()
                .filter(|(result, _)| result.count == largest)
                .map(|(result, value)| RankEntry {
                    variant: result.variant,
                    value: *value,
                    flags: Vec::new(),
                })
                .collect(),
        )
    }
}

/// Trace area rankings from core proofs.
struct AreaRanking {
    title: String,
    net: Vec<RankEntry>,
    /// Needs a fit, because area divided by one count includes all the fixed overhead. It uses each
    /// guest's own cost and not the baseline, whose loop shape would swamp a Poseidon2 call.
    real_node: Option<Vec<RankEntry>>,
}

/// With several counts, area is fitted, which separates out the fixed cost of an extra shard
/// exactly, because area is deterministic.
fn area_ranking(rows: &[ProofRow], tier: Tier, size: usize) -> Option<AreaRanking> {
    let points = |variant: Variant| -> Vec<(f64, f64)> {
        rows.iter()
            .filter(|row| row.variant == variant && row.data_size == size && row.mode == Mode::Core)
            .filter_map(|row| Some((row.count as f64, row.trace_area? as f64)))
            .collect()
    };
    let baseline = points(Variant::Baseline);
    let same_counts = |own: &[(f64, f64)]| {
        own.len() == baseline.len() && own.iter().zip(&baseline).all(|(a, b)| a.0 == b.0)
    };
    let variants: Vec<_> = Variant::ALL
        .into_iter()
        .filter(|variant| variant.tier() == tier && !points(*variant).is_empty())
        .collect();
    if baseline.is_empty() || variants.is_empty() {
        return None;
    }

    let entry = |variant, value| RankEntry {
        variant,
        value,
        flags: Vec::new(),
    };
    if baseline.len() >= 2
        && variants
            .iter()
            .all(|variant| same_counts(&points(*variant)))
    {
        let baseline_slope = fit_line(&baseline).1;
        let slopes: Vec<_> = variants
            .iter()
            .map(|variant| (*variant, fit_line(&points(*variant)).1))
            .collect();
        return Some(AreaRanking {
            title: format!(
                "Main trace area per hash, net of baseline, fitted across {} counts. Good to a few \
                 hundred, which matters only for Poseidon2",
                baseline.len()
            ),
            net: slopes
                .iter()
                .map(|(variant, slope)| entry(*variant, slope - baseline_slope))
                .collect(),
            real_node: Some(
                slopes
                    .iter()
                    .map(|(variant, slope)| entry(*variant, slope - FRESH_CHILD_AREA))
                    .collect(),
            ),
        });
    }

    let largest = rows
        .iter()
        .filter(|row| {
            row.variant.tier() == tier
                && row.data_size == size
                && row.mode == Mode::Core
                && row.net_area_per_hash.is_some()
        })
        .map(|row| row.count)
        .max()?;
    Some(AreaRanking {
        title: format!(
            "Main trace area per hash, net of baseline, x = {largest}, including the fixed cost of \
             any extra shard. Good to a few hundred, which matters only for Poseidon2"
        ),
        net: rows
            .iter()
            .filter(|row| {
                row.variant.tier() == tier
                    && row.data_size == size
                    && row.mode == Mode::Core
                    && row.count == largest
            })
            .filter_map(|row| Some(entry(row.variant, row.net_area_per_hash?)))
            .collect(),
        real_node: None,
    })
}

/// Ranks prove time, on a fit across counts when there is one.
fn time_ranking(
    rows: &[ProofRow],
    slopes: &[Slope],
    tier: Tier,
    size: usize,
    mode: Mode,
) -> Option<String> {
    let fitted: Vec<_> = slopes
        .iter()
        .filter(|slope| {
            slope.variant.tier() == tier && slope.data_size == size && slope.mode == mode
        })
        .collect();
    let (title, entries): (_, Vec<_>) = if fitted.is_empty() {
        (
            format!(
                "Prove time per hash, {}, net of baseline, ms, one count so it includes fixed shard overhead",
                mode.name()
            ),
            rows.iter()
                .filter(|row| {
                    row.variant.tier() == tier
                        && row.data_size == size
                        && row.mode == mode
                        && row.warm_samples >= MIN_RANKED_SAMPLES
                })
                .filter_map(|row| {
                    Some(RankEntry {
                        variant: row.variant,
                        value: row.net_ms_per_hash?,
                        flags: row.flags.clone(),
                    })
                })
                .collect(),
        )
    } else {
        (
            format!(
                "Prove time per hash, {}, slope net of baseline, ms",
                mode.name()
            ),
            fitted
                .iter()
                .filter(|slope| slope.warm_samples >= MIN_RANKED_SAMPLES)
                .filter_map(|slope| {
                    Some(RankEntry {
                        variant: slope.variant,
                        value: slope.net_ms_per_hash?,
                        flags: slope.flags.clone(),
                    })
                })
                .collect(),
        )
    };
    (!entries.is_empty()).then(|| ranked(&title, entries, 3))
}

/// Timing for a variant at one point, with any repeated baseline runs combined.
struct TimeStats {
    cold_ms: f64,
    /// The range of the cold proofs, which is only the drift between repeated baseline runs.
    cold_drift_ms: f64,
    warm: Option<WarmStats>,
    samples: usize,
}

#[derive(Clone, Copy)]
struct WarmStats {
    median_ms: f64,
    /// Max minus min of the warm runs, plus the drift between repeated baseline runs.
    spread_ms: f64,
}

impl TimeStats {
    fn of(results: &[&ProofResult]) -> Self {
        let colds: Vec<f64> = results
            .iter()
            .map(|result| result.runs[0].prove_ms)
            .collect();
        let samples = results.iter().map(|result| result.warm_runs().len()).sum();
        let warm = (samples > 0).then(|| {
            let medians: Vec<f64> = results
                .iter()
                .map(|result| median(&warm_times(result)))
                .collect();
            let widest = results
                .iter()
                .map(|result| range(&warm_times(result)))
                .fold(0.0, f64::max);
            WarmStats {
                median_ms: mean(&medians),
                spread_ms: widest + range(&medians),
            }
        });
        Self {
            cold_ms: mean(&colds),
            cold_drift_ms: range(&colds),
            warm,
            samples,
        }
    }

    /// The time net figures use: warm if there are warm proofs, otherwise cold.
    fn basis_ms(&self) -> f64 {
        self.warm.map_or(self.cold_ms, |warm| warm.median_ms)
    }

    fn spread_ms(&self) -> f64 {
        self.warm.map_or(self.cold_drift_ms, |warm| warm.spread_ms)
    }
}

fn warm_times(result: &ProofResult) -> Vec<f64> {
    result.warm_runs().iter().map(|run| run.prove_ms).collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn range(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::MIN, f64::max)
        - values.iter().copied().fold(f64::MAX, f64::min)
}

/// Fits a line through each variant's times across the counts of a sweep. A net figure needs the
/// variant and baseline to share their counts, which a partial report may not.
pub fn slopes(rows: &[ProofRow]) -> Vec<Slope> {
    let mut groups: BTreeMap<(usize, Mode, Variant), Vec<&ProofRow>> = BTreeMap::new();
    for row in rows {
        groups
            .entry((row.data_size, row.mode, row.variant))
            .or_default()
            .push(row);
    }
    let counts_of = |key: &(usize, Mode, Variant)| -> Vec<usize> {
        groups
            .get(key)
            .map_or_else(Vec::new, |rows| rows.iter().map(|row| row.count).collect())
    };
    let fits: BTreeMap<_, _> = groups
        .iter()
        .filter(|(_, rows)| rows.len() >= 2)
        .map(|(key, rows)| {
            let points: Vec<_> = rows
                .iter()
                .map(|row| (row.count as f64, row.warm_median_ms.unwrap_or(row.cold_ms)))
                .collect();
            (*key, fit_line(&points))
        })
        .collect();
    fits.iter()
        .map(|(&key, &(fixed_ms, ms_per_hash))| {
            let (data_size, mode, variant) = key;
            let baseline = (data_size, mode, Variant::Baseline);
            let mut flags: Vec<_> = groups[&key]
                .iter()
                .flat_map(|row| row.flags.iter().copied())
                .collect();
            flags.sort_unstable();
            flags.dedup();
            Slope {
                variant,
                data_size,
                mode,
                counts: groups[&key].len(),
                fixed_ms,
                ms_per_hash,
                net_ms_per_hash: (variant != Variant::Baseline
                    && counts_of(&key) == counts_of(&baseline))
                .then(|| fits.get(&baseline).map(|fit| ms_per_hash - fit.1))
                .flatten(),
                warm_samples: groups[&key]
                    .iter()
                    .map(|row| row.warm_samples)
                    .min()
                    .unwrap_or(0),
                flags,
            }
        })
        .collect()
}

/// Least squares: returns the intercept and slope.
fn fit_line(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let mean_x = points.iter().map(|point| point.0).sum::<f64>() / n;
    let mean_y = points.iter().map(|point| point.1).sum::<f64>() / n;
    let covariance: f64 = points
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum();
    let variance: f64 = points.iter().map(|(x, _)| (x - mean_x).powi(2)).sum();
    let slope = covariance / variance;
    (mean_y - slope * mean_x, slope)
}

fn print_proofs(rows: &[ProofRow]) {
    if rows.is_empty() {
        return;
    }
    let mut table = new_table([
        "variant",
        "mode",
        "bytes",
        "x",
        "setup s",
        "prove s (cold)",
        "prove s (warm)",
        "spread s",
        "net ms/hash",
        "shards",
        "area",
        "net area/hash",
        "proof KiB",
        "verify ms",
        "flags",
    ]);
    for row in rows {
        table.add_row([
            row.variant.name().to_string(),
            row.mode.name().to_string(),
            row.data_size.to_string(),
            row.count.to_string(),
            format!("{:.1}", row.setup_ms / 1e3),
            format!("{:.1}", row.cold_ms / 1e3),
            row.warm_median_ms
                .map_or("-".to_string(), |value| format!("{:.1}", value / 1e3)),
            row.warm_spread_ms
                .map_or("-".to_string(), |value| format!("{:.1}", value / 1e3)),
            row.net_ms_per_hash
                .map_or("-".to_string(), |value| format!("{value:.3}")),
            row.shards
                .map_or("-".to_string(), |value| value.to_string()),
            row.trace_area
                .map_or("-".to_string(), |value| value.to_string()),
            format_net(row.net_area_per_hash),
            format!("{:.1}", row.proof_bytes as f64 / 1024.0),
            format!("{:.1}", row.verify_median_ms),
            row.flags.join(","),
        ]);
    }
    println!(
        "\nProving. Flags mark prove times to distrust: cold-only = one proof, so no warm timing and \
         net time includes first-proof costs; few-samples = under {MIN_RANKED_SAMPLES} warm proofs; \
         busy = other processes used over {BUSY_CORES} core, here or in the baseline; swap = swap grew, \
         here or in the baseline; shards = shard count differs from baseline, so net time includes \
         whole-shard overhead; noise = delta under 2x the spread.\n{table}"
    );
}

fn print_slopes(slopes: &[Slope]) {
    if slopes.is_empty() {
        return;
    }
    let mut table = new_table([
        "variant",
        "mode",
        "bytes",
        "counts",
        "fixed s",
        "ms/hash",
        "net ms/hash",
        "warm proofs",
        "flags",
    ]);
    for slope in slopes {
        table.add_row([
            slope.variant.name().to_string(),
            slope.mode.name().to_string(),
            slope.data_size.to_string(),
            slope.counts.to_string(),
            format!("{:.1}", slope.fixed_ms / 1e3),
            format!("{:.3}", slope.ms_per_hash),
            slope
                .net_ms_per_hash
                .map_or("-".to_string(), |value| format!("{value:.3}")),
            slope.warm_samples.to_string(),
            slope.flags.join(","),
        ]);
    }
    println!("\nProve time fitted across counts: time = fixed + ms/hash x count.\n{table}");
}

#[derive(Clone)]
struct RankEntry {
    variant: Variant,
    value: f64,
    flags: Vec<&'static str>,
}

/// Formats a ranking, cheapest first, with each value as a multiple of the cheapest. A multiple of
/// a value that is itself within noise would mean nothing, so none is given.
fn ranked(title: &str, mut entries: Vec<RankEntry>, decimals: usize) -> String {
    entries.sort_by(|a, b| a.value.total_cmp(&b.value));
    let best = entries.first().map_or(0.0, |entry| entry.value);
    let best_is_noise = entries
        .first()
        .is_some_and(|entry| entry.flags.contains(&"noise"));
    let mut text = format!("  {title}");
    for (place, entry) in entries.iter().enumerate() {
        let multiple = if best_is_noise {
            "within noise".to_string()
        } else if best > 0.0 {
            format!("{:.1}x", entry.value / best)
        } else {
            "-".to_string()
        };
        let flags = if entry.flags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", entry.flags.join(","))
        };
        text.push_str(&format!(
            "\n    {}. {:<18} {:>12.decimals$}  {multiple}{flags}",
            place + 1,
            entry.variant.name(),
            entry.value
        ));
    }
    text
}

fn format_net(value: Option<f64>) -> String {
    value.map_or("-".to_string(), |value| format!("{value:.0}"))
}

fn new_table<const N: usize>(header: [&str; N]) -> Table {
    let mut table = Table::new();
    table.load_style(UTF8_FULL_CONDENSED).set_header(header);
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_handles_odd_and_even_lengths() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    fn proof(variant: Variant, count: usize, area: u64, times: &[f64]) -> ProofResult {
        ProofResult {
            variant,
            data_size: 64,
            count,
            mode: Mode::Core,
            setup_ms: 1.0,
            runs: times
                .iter()
                .map(|time| ProofRun {
                    prove_ms: *time,
                    load: LoadSample::default(),
                })
                .collect(),
            proof_bytes: 1,
            verify_ms: vec![1.0],
            negative_controls_passed: Some(true),
            shards: Some(2),
            trace_area: Some(area),
            chip_area: BTreeMap::new(),
        }
    }

    #[test]
    fn a_single_proof_is_cold_only_and_never_warm() {
        let result = proof(Variant::Sha256Node, 100, 10, &[500.0]);
        let time = TimeStats::of(&[&result]);
        assert_eq!(time.samples, 0);
        assert!(time.warm.is_none());
        assert_eq!(time.basis_ms(), 500.0);
    }

    #[test]
    fn warm_timing_leaves_out_the_first_proof() {
        let result = proof(Variant::Sha256Node, 100, 10, &[900.0, 100.0, 300.0, 200.0]);
        let time = TimeStats::of(&[&result]);
        assert_eq!(time.samples, 3);
        assert_eq!(time.cold_ms, 900.0);
        assert_eq!(time.basis_ms(), 200.0);
        assert_eq!(time.spread_ms(), 200.0);
    }

    #[test]
    fn repeated_baseline_runs_count_their_drift_as_spread() {
        let first = proof(Variant::Baseline, 100, 10, &[0.0, 100.0, 100.0, 100.0]);
        let last = proof(Variant::Baseline, 100, 10, &[0.0, 150.0, 150.0, 150.0]);
        let time = TimeStats::of(&[&first, &last]);
        assert_eq!(time.basis_ms(), 125.0);
        assert_eq!(time.spread_ms(), 50.0);
    }

    #[test]
    fn area_ranking_fits_out_fixed_overhead_across_counts() {
        // An extra shard costs a fixed 1000 here, which must not reach the per-hash figure.
        let results = [
            proof(Variant::Baseline, 100, 10_000, &[1.0]),
            proof(Variant::Baseline, 200, 20_000, &[1.0]),
            proof(Variant::Keccak256Node, 100, 31_000, &[1.0]),
            proof(Variant::Keccak256Node, 200, 51_000, &[1.0]),
        ];
        let report = Report {
            meta: test_meta(),
            native: Vec::new(),
            execution: Vec::new(),
            proofs: results.into_iter().collect(),
        };
        let area = area_ranking(&report.proof_rows(), Tier::Node, 64).unwrap();
        assert!(area.title.contains("fitted"));
        assert_eq!(area.net.len(), 1);
        assert!((area.net[0].value - 100.0).abs() < 1e-9);
        // The estimate uses the guest's own 200 per hash, not the baseline's 100.
        let real_node = area.real_node.unwrap();
        assert!((real_node[0].value - (200.0 - FRESH_CHILD_AREA)).abs() < 1e-9);
    }

    #[test]
    fn one_count_gives_no_real_node_estimate() {
        let report = Report {
            meta: test_meta(),
            native: Vec::new(),
            execution: Vec::new(),
            proofs: vec![
                proof(Variant::Baseline, 100, 10_000, &[1.0]),
                proof(Variant::Keccak256Node, 100, 31_000, &[1.0]),
            ],
        };
        let area = area_ranking(&report.proof_rows(), Tier::Node, 64).unwrap();
        assert!((area.net[0].value - 210.0).abs() < 1e-9);
        assert!(area.real_node.is_none());
    }

    #[test]
    fn no_multiples_against_a_value_within_noise() {
        let entries = vec![
            RankEntry {
                variant: Variant::Poseidon2Node,
                value: 0.03,
                flags: vec!["noise"],
            },
            RankEntry {
                variant: Variant::Sha256Node,
                value: 21.0,
                flags: Vec::new(),
            },
        ];
        let text = ranked("title", entries, 3);
        assert!(text.contains("within noise"));
        assert!(!text.contains("700.0x"));
    }

    fn test_meta() -> Meta {
        Meta {
            sp1_version: SP1_VERSION,
            circuit_version: SP1_CIRCUIT_VERSION,
            host_rustc: "",
            guest_toolchain: GUEST_TOOLCHAIN,
            os: "",
            arch: "",
            cpu: String::new(),
            cores: 1,
            memory_gib: 1,
            prover: String::new(),
            seed: 0,
            data_sizes: Vec::new(),
            counts: Vec::new(),
            variants: Vec::new(),
            modes: Vec::new(),
            runs: 1,
            debug_build: false,
            complete: false,
            unix_time: 0,
            env_overrides: BTreeMap::new(),
            run_order: Vec::new(),
            programs: Vec::new(),
        }
    }

    #[test]
    fn fit_line_recovers_a_line() {
        let (intercept, slope) = fit_line(&[(100.0, 1200.0), (200.0, 1400.0), (400.0, 1800.0)]);
        assert!((intercept - 1000.0).abs() < 1e-9);
        assert!((slope - 2.0).abs() < 1e-9);
    }
}

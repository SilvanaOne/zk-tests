//! The measurements: native hashing, guest execution, proving and verification.

use std::{
    collections::BTreeMap,
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use rand::{SeedableRng, rngs::ChaCha20Rng, seq::SliceRandom};
use sha2::{Digest, Sha256};
use slop_algebra::PrimeField32;
use sp1_sdk::{
    ExecutionReport, HashableKey, LightProver, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1Proof, SP1ProofWithPublicValues, SP1PublicValues, SP1VerifyingKey,
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{
    cli::{Config, Mode},
    data::Dataset,
    native,
    report::{
        ExecutionResult, NativeResult, ProgramInfo, ProofResult, ProofRun, Report, median,
        prover_name,
    },
    variant::{SyscallCounts, Variant},
};

/// Native samples per variant.
const NATIVE_SAMPLES: usize = 5;
/// Each native sample repeats until it lasts this long, to average out clock and scheduler effects.
const NATIVE_MIN_SAMPLE: Duration = Duration::from_millis(200);
/// Verifications per proof.
const VERIFY_SAMPLES: usize = 5;
/// How often to sample CPU use by other processes during a proof.
const LOAD_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Runs every measurement a configuration asks for.
pub async fn run(config: &Config) -> Result<Report> {
    let mut report = Report::new(config)?;
    report.print_header();
    print_skipped_variants(config);

    // Native first, so no prover state or thread pool exists while the timing runs.
    run_native(config, &mut report)?;

    let light = LightProver::new().await;
    run_execution(config, &light, &mut report).await?;
    if !config.execute_only {
        run_proving(config, &light, &mut report).await?;
    }
    report.meta.complete = true;
    report.save(config)?;
    Ok(report)
}

fn print_skipped_variants(config: &Config) {
    for size in &config.data_sizes {
        let skipped: Vec<_> = config
            .variants
            .iter()
            .filter(|variant| !variant.runs_at(*size))
            .map(|variant| variant.name())
            .collect();
        if !skipped.is_empty() {
            println!(
                "Skipping {} at {size} bytes: node variants only run at 64 bytes.",
                skipped.join(", ")
            );
        }
    }
}

/// All `(size, count)` points of the sweep.
fn points(config: &Config) -> Vec<(usize, usize)> {
    let mut points = Vec::new();
    for size in &config.data_sizes {
        for count in &config.counts {
            points.push((*size, *count));
        }
    }
    points
}

fn run_native(config: &Config, report: &mut Report) -> Result<()> {
    println!("\nNative hashing (single thread)...");
    for (size, count) in points(config) {
        let data = Dataset::generate(config.seed, size, count);
        for variant in config.variants.iter().filter(|v| v.runs_at(size)) {
            report.native.push(time_native(*variant, &data));
        }
    }
    report.save(config)
}

/// Times one variant over the whole dataset, in nanoseconds per hash.
pub fn time_native(variant: Variant, data: &Dataset) -> NativeResult {
    black_box(native::accumulate(variant, data));

    let mut samples = Vec::with_capacity(NATIVE_SAMPLES);
    for _ in 0..NATIVE_SAMPLES {
        let started = Instant::now();
        let mut hashes = 0u64;
        while started.elapsed() < NATIVE_MIN_SAMPLE {
            black_box(native::accumulate(variant, black_box(data)));
            hashes += data.count as u64;
        }
        samples.push(started.elapsed().as_nanos() as f64 / hashes as f64);
    }

    let median_ns = median(&samples);
    NativeResult {
        variant,
        data_size: data.size,
        count: data.count,
        median_ns_per_hash: median_ns,
        min_ns_per_hash: samples.iter().copied().fold(f64::INFINITY, f64::min),
        mb_per_s: data.size as f64 / median_ns * 1e3,
        samples_ns_per_hash: samples,
    }
}

async fn run_execution(config: &Config, light: &LightProver, report: &mut Report) -> Result<()> {
    println!("Executing guests...");
    for variant in &config.variants {
        report
            .meta
            .programs
            .push(program_info(light, *variant).await?);
    }
    for (size, count) in points(config) {
        let data = Dataset::generate(config.seed, size, count);
        for variant in config.variants.iter().filter(|v| v.runs_at(size)) {
            report
                .execution
                .push(execute_and_check(light, *variant, &data).await?);
        }
    }
    report.save(config)
}

async fn program_info(light: &LightProver, variant: Variant) -> Result<ProgramInfo> {
    let elf = variant.elf();
    let pk = light
        .setup(elf.clone())
        .await
        .map_err(|error| anyhow!("{error}"))
        .with_context(|| format!("setting up {}", variant.name()))?;
    Ok(ProgramInfo {
        variant,
        elf_bytes: elf.len(),
        elf_sha256: hex(Sha256::digest(&*elf).as_slice()),
        vkey: pk.verifying_key().bytes32(),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Executes one guest and fails unless it computed the native result with the expected precompile
/// calls. This runs before any proving, so a broken guest cannot waste a long proof or produce a
/// number that measures the wrong thing.
pub async fn execute_and_check(
    light: &LightProver,
    variant: Variant,
    data: &Dataset,
) -> Result<ExecutionResult> {
    let (public_values, execution) = light
        .execute(variant.elf(), data.stdin(variant))
        .await
        .map_err(|error| anyhow!("{error}"))
        .with_context(|| format!("executing {} at {} bytes", variant.name(), data.size))?;

    ensure!(
        execution.exit_code == 0,
        "{} exited with code {} at {} bytes",
        variant.name(),
        execution.exit_code,
        data.size
    );
    ensure!(
        public_values.as_slice() == data.expected_output(variant),
        "{} committed {} at {} bytes, but the native hash gives {}",
        variant.name(),
        hex(public_values.as_slice()),
        data.size,
        hex(&data.expected_output(variant))
    );
    let syscalls = SyscallCounts::from_report(&execution);
    let expected = variant.expected_syscalls(data.size, data.count);
    ensure!(
        syscalls == expected,
        "{} made {syscalls:?} at {} bytes, expected {expected:?}. A precompile patch was probably \
         dropped: check program/Cargo.lock",
        variant.name(),
        data.size
    );

    Ok(execution_result(variant, data, &execution, syscalls))
}

fn execution_result(
    variant: Variant,
    data: &Dataset,
    execution: &ExecutionReport,
    syscalls: SyscallCounts,
) -> ExecutionResult {
    ExecutionResult {
        variant,
        data_size: data.size,
        count: data.count,
        cycles: execution.total_instruction_count(),
        gas: execution.gas(),
        syscalls,
        opcodes: execution
            .opcode_counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(opcode, count)| (format!("{opcode:?}"), *count))
            .collect(),
    }
}

async fn run_proving(config: &Config, light: &LightProver, report: &mut Report) -> Result<()> {
    // A verifying key from another program, to show verification checks the proof. Baseline's
    // proofs get the SHA-256 key, and every other variant's get baseline's.
    let baseline_vk = verifying_key(light, Variant::Baseline).await?;
    let other_vk = verifying_key(light, Variant::Sha256).await?;

    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    for (size, count) in points(config) {
        let data = Dataset::generate(config.seed, size, count);
        for mode in &config.modes {
            let decoy = |variant| {
                if variant == Variant::Baseline {
                    &other_vk
                } else {
                    &baseline_vk
                }
            };
            for variant in run_order(config, size, &mut rng) {
                println!(
                    "\nProving {} ({}, {size} bytes x {count}, {} run{})...",
                    variant.name(),
                    mode.name(),
                    config.runs,
                    if config.runs == 1 { "" } else { "s" }
                );
                report.meta.run_order.push(variant.name());
                let result =
                    measure_proofs(variant, *mode, &data, decoy(variant), config.runs).await?;
                println!("  {}", result.summary());
                report.proofs.push(result);
                report.save(config)?;
            }
        }
    }
    Ok(())
}

async fn verifying_key(light: &LightProver, variant: Variant) -> Result<SP1VerifyingKey> {
    let pk = light
        .setup(variant.elf())
        .await
        .map_err(|error| anyhow!("{error}"))?;
    Ok(pk.verifying_key().clone())
}

/// Baseline first and last, so its drift across the sweep point can be seen, with the others in a
/// seeded random order so no hash always runs after the same one.
fn run_order(config: &Config, size: usize, rng: &mut ChaCha20Rng) -> Vec<Variant> {
    let mut others: Vec<_> = config
        .variants
        .iter()
        .copied()
        .filter(|variant| variant.runs_at(size) && *variant != Variant::Baseline)
        .collect();
    others.shuffle(rng);
    if config.variants.contains(&Variant::Baseline) {
        others.insert(0, Variant::Baseline);
        if others.len() > 1 {
            others.push(Variant::Baseline);
        }
    }
    others
}

/// Proves and verifies one variant with a fresh client, so caches kept per client (such as the
/// compiled recursion programs for each shard shape) cannot carry over between hashes.
async fn measure_proofs(
    variant: Variant,
    mode: Mode,
    data: &Dataset,
    decoy_vk: &SP1VerifyingKey,
    runs: usize,
) -> Result<ProofResult> {
    let client = ProverClient::from_env().await;

    let started = Instant::now();
    let pk = client
        .setup(variant.elf())
        .await
        .with_context(|| format!("setting up {}", variant.name()))?;
    let setup_ms = millis(started);

    let mut proof_runs = Vec::with_capacity(runs);
    let mut proof = None;
    for _ in 0..runs {
        // Free the previous proof first, and build the input outside the timer.
        drop(proof.take());
        let stdin = data.stdin(variant);
        let monitor = LoadMonitor::start();
        let started = Instant::now();
        let result = client.prove(&pk, stdin).mode(mode.proof_mode()).await;
        let prove_ms = millis(started);
        let load = monitor.finish();
        proof = Some(result.with_context(|| {
            format!(
                "proving {} ({}) at {} bytes x {}",
                variant.name(),
                mode.name(),
                data.size,
                data.count
            )
        })?);
        proof_runs.push(ProofRun { prove_ms, load });
    }
    let mut proof = proof.context("--runs is at least 1")?;

    ensure!(
        proof.public_values.as_slice() == data.expected_output(variant),
        "the proof of {} committed public values that differ from the native hash",
        variant.name()
    );

    let vk = pk.verifying_key();
    let mut verify_ms = Vec::with_capacity(VERIFY_SAMPLES);
    for _ in 0..VERIFY_SAMPLES {
        let started = Instant::now();
        client
            .verify(&proof, vk, None)
            .with_context(|| format!("verifying the proof of {}", variant.name()))?;
        verify_ms.push(millis(started));
    }

    // The mock prover accepts everything, so these only mean something for real proofs.
    let negative_controls_passed = if prover_name() == "mock" {
        None
    } else {
        check_rejections(&client, &mut proof, vk, decoy_vk)?;
        Some(true)
    };

    let (shards, chip_area) = trace_details(&proof);
    Ok(ProofResult {
        variant,
        data_size: data.size,
        count: data.count,
        mode,
        setup_ms,
        runs: proof_runs,
        proof_bytes: bincode::serialized_size(&proof)?,
        verify_ms,
        negative_controls_passed,
        shards,
        trace_area: chip_area.as_ref().map(|chips| chips.values().sum()),
        chip_area: chip_area.unwrap_or_default(),
    })
}

/// Requires verification to fail for changed public values and for another program's key. Public
/// values are checked before the proof itself, so the key is what exercises the proof check.
fn check_rejections(
    client: &impl Prover,
    proof: &mut SP1ProofWithPublicValues,
    vk: &SP1VerifyingKey,
    decoy_vk: &SP1VerifyingKey,
) -> Result<()> {
    ensure!(
        client.verify(proof, decoy_vk, None).is_err(),
        "verification accepted a proof under another program's key"
    );

    let mut tampered = proof.public_values.to_vec();
    tampered[0] ^= 1;
    let original = std::mem::replace(&mut proof.public_values, SP1PublicValues::from(&tampered));
    let accepted = client.verify(proof, vk, None).is_ok();
    proof.public_values = original;
    ensure!(!accepted, "verification accepted changed public values");
    Ok(())
}

/// The shard count and the main trace area per chip, which only a core proof exposes. The area
/// counts real rows, so unlike gas it includes all memory and precompile traffic. Preprocessed
/// columns are left out. Mock proofs have no shards, so they report nothing.
fn trace_details(
    proof: &SP1ProofWithPublicValues,
) -> (Option<usize>, Option<BTreeMap<String, u64>>) {
    let SP1Proof::Core(shards) = &proof.proof else {
        return (None, None);
    };
    if shards.is_empty() {
        return (None, None);
    }
    let mut chip_area = BTreeMap::new();
    for shard in shards {
        for (name, chip) in &shard.opened_values.chips {
            // The degree is stored as big-endian bits.
            let height = chip.degree.iter().fold(0u64, |height, bit| {
                height << 1 | bit.as_canonical_u32() as u64
            });
            *chip_area.entry(name.clone()).or_default() += height * chip.main.local.len() as u64;
        }
    }
    (Some(shards.len()), Some(chip_area))
}

fn millis(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1e3
}

/// Conditions during one proof that could disturb its timing.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct LoadSample {
    /// Mean CPU used by other processes during the proof, in cores, or None if it was not sampled.
    /// The system load average is no use here, because it includes the prover.
    pub other_cpu_cores: Option<f64>,
    pub swap_grew_bytes: u64,
}

/// Samples CPU use by everything except this process while a proof runs.
struct LoadMonitor {
    stop: Arc<AtomicBool>,
    sampler: thread::JoinHandle<Option<f64>>,
    swap_before: u64,
}

impl LoadMonitor {
    fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let sampler = {
            let stop = stop.clone();
            thread::spawn(move || sample_other_cpu(&stop))
        };
        Self {
            stop,
            sampler,
            swap_before: used_swap(),
        }
    }

    fn finish(self) -> LoadSample {
        self.stop.store(true, Ordering::Relaxed);
        LoadSample {
            other_cpu_cores: self.sampler.join().ok().flatten(),
            swap_grew_bytes: used_swap().saturating_sub(self.swap_before),
        }
    }
}

fn used_swap() -> u64 {
    let Ok(mut system) = System::new() else {
        return 0;
    };
    system.refresh_memory();
    system.used_swap()
}

fn sample_other_cpu(stop: &AtomicBool) -> Option<f64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = System::new().ok()?;
    refresh_cpu(&mut system, pid);

    let (mut total, mut samples) = (0.0, 0u32);
    while !stop.load(Ordering::Relaxed) {
        // Sleep in short steps, so a short proof is not held up after it finishes.
        let started = Instant::now();
        while started.elapsed() < LOAD_SAMPLE_INTERVAL && !stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
        }
        if started.elapsed() < sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
            break;
        }
        refresh_cpu(&mut system, pid);
        let cores = system.cpus().len() as f64;
        let all = system.global_cpu_usage() as f64 / 100.0 * cores;
        let own = system
            .process(pid)
            .map_or(0.0, |process| process.cpu_usage() as f64 / 100.0);
        total += (all - own).max(0.0);
        samples += 1;
    }
    (samples > 0).then(|| total / samples as f64)
}

fn refresh_cpu(system: &mut System, pid: Pid) {
    system.refresh_cpu_usage();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );
}

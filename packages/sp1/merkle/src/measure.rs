//! Measures one point in a child process, so a proof that runs too long can be stopped, and its
//! memory peak is its own.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Write,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use merkle_map::record::PublicValues;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sp1_core_executor::SyscallCode;
use sp1_sdk::{
    ExecutionReport, HashableKey, LightProver, ProveRequest, Prover, ProverClient, ProvingKey,
    SP1ProofMode, SP1PublicValues, SP1Stdin,
};

use crate::{
    arm::Arm,
    point::{Point, RunMode},
    prepare::Sidecar,
    proof::{LoadMonitor, LoadSample, LoadWatch, check_rejections, trace_details},
    software::{SyscallCounts, take_counts},
};

const VERIFY_SAMPLES: usize = 5;
pub const RUNS_PATH: &str = "results/runs.jsonl";
/// A proof is stopped after this long at most. Stopping is a result too.
const MAX_PROOF: Duration = Duration::from_secs(4 * 60 * 60);
/// A high estimate of main trace cells per gas across these guests, for the time limit below.
const CELLS_PER_GAS: f64 = 100.0;

/// How long a proof may run: three times the time model for its estimated size, up to the maximum.
/// The model is `12.7 s + 3.5 s per shard over 2 + 0.133 s per million cells`.
fn proof_limit(gas: Option<u64>, mode: RunMode) -> Duration {
    let cells = gas.unwrap_or_default() as f64 * CELLS_PER_GAS;
    let shards = (cells / 402_653_184.0).ceil().max(1.0) + 1.0;
    let model = 12.7 + 3.5 * (shards - 2.0) + 0.133 * cells / 1e6;
    // Compression adds recursion over every shard's proof, which the model does not cover.
    let compression = if mode == RunMode::Compressed {
        60.0 * shards
    } else {
        0.0
    };
    Duration::from_secs_f64(3.0 * model.max(120.0) + compression).min(MAX_PROOF)
}

/// One measurement, as one line of the results file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Run {
    pub run_id: String,
    pub point: Option<Point>,
    pub status: String,
    pub error: Option<String>,
    pub elf_sha256: String,
    pub witness_sha256: String,
    pub vkey: String,
    pub env: BTreeMap<String, String>,
    pub sidecar: Option<Sidecar>,
    pub native_ms: f64,
    pub counts: Option<SyscallCounts>,
    pub cycles: u64,
    pub gas: Option<u64>,
    pub execute_ms: f64,
    pub opcodes: BTreeMap<String, u64>,
    pub setup_ms: Option<f64>,
    pub prove_ms: Option<f64>,
    pub load: Option<LoadSample>,
    pub verify_ms: Option<f64>,
    pub proof_bytes: Option<u64>,
    pub shards: Option<usize>,
    pub trace_area: Option<u64>,
    pub chip_area: BTreeMap<String, u64>,
    /// Resident memory at its peak, which falls when the system compresses or swaps the process.
    pub peak_rss_bytes: Option<u64>,
    /// The memory the process needed at its peak, wherever it was. macOS only.
    pub peak_footprint_bytes: Option<u64>,
    /// How long the proof was allowed.
    pub limit_s: Option<f64>,
    /// How long the whole measurement took.
    pub wall_s: Option<f64>,
}

/// How a proof stopped for memory describes itself.
const MEMORY_STOP: &str = "needed more than";

impl Run {
    pub fn stopped_for_memory(&self) -> bool {
        self.status == "stopped"
            && self
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with(MEMORY_STOP))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    hex(&digest)
}

/// Copies a witness into 8-aligned memory.
pub fn aligned(bytes: &[u8]) -> Vec<u64> {
    assert!(bytes.len().is_multiple_of(8));
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_ne_bytes(*chunk))
        .collect()
}

pub fn as_bytes(words: &[u64]) -> &[u8] {
    // SAFETY: any `u64`s are valid bytes, and the length covers exactly the same memory.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 8) }
}

/// The header field that sets the stop point, patched into a copy of the witness.
const STOP_AFTER_OFFSET: usize = 20;

pub fn load_witness(point: &Point) -> Result<Vec<u64>> {
    let path = point.witness_path();
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut words = aligned(&bytes);
    // SAFETY: the buffer holds at least the header, and the offset is 4-aligned within it.
    unsafe {
        words
            .as_mut_ptr()
            .cast::<u8>()
            .add(STOP_AFTER_OFFSET)
            .cast::<u32>()
            .write(point.stop_after);
    }
    Ok(words)
}

fn syscalls(report: &ExecutionReport) -> SyscallCounts {
    SyscallCounts {
        sha_extend: report.syscall_counts[SyscallCode::SHA_EXTEND],
        sha_compress: report.syscall_counts[SyscallCode::SHA_COMPRESS],
        poseidon2: report.syscall_counts[SyscallCode::POSEIDON2],
    }
}

/// SP1 hashes the public values with SHA-256 inside the guest: three blocks for 152 bytes.
const PUBLIC_VALUE_BLOCKS: u64 = 3;

/// Runs a guest's engine on the host, for the output and precompile calls the guest must match.
pub(crate) fn expected_run(arm: Arm, witness: &[u8]) -> (PublicValues, SyscallCounts) {
    take_counts();
    let expected = arm.native(witness);
    let mut counts = take_counts();
    counts.sha_extend += PUBLIC_VALUE_BLOCKS;
    counts.sha_compress += PUBLIC_VALUE_BLOCKS;
    (expected, counts)
}

/// A guest run has to exit cleanly and match the host's output and precompile calls.
pub(crate) fn check_execution(
    report: &ExecutionReport,
    public_values: &SP1PublicValues,
    expected: &PublicValues,
    counts: SyscallCounts,
) -> Result<()> {
    ensure!(report.exit_code == 0, "exit code {}", report.exit_code);
    ensure!(
        public_values.as_slice() == expected.as_bytes(),
        "the guest committed {} but the host computed {}",
        hex(public_values.as_slice()),
        hex(expected.as_bytes())
    );
    let actual = syscalls(report);
    ensure!(
        actual == counts,
        "the guest made {actual:?}, the host expected {counts:?}. A precompile patch was probably \
         dropped: check program/Cargo.lock"
    );
    Ok(())
}

/// How long a panic elsewhere may go unanswered. A panic in a prover task leaves the proof waiting
/// forever, so it has to end the run.
const PANIC_GRACE: Duration = Duration::from_secs(60);

static PANIC: Mutex<Option<String>> = Mutex::new(None);

fn record_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut panic) = PANIC.lock() {
            panic.get_or_insert_with(|| info.to_string());
        }
        default(info);
    }));
}

/// The largest physical footprint of a process so far. Unlike resident memory, it includes what the
/// system has compressed or swapped out.
#[cfg(target_os = "macos")]
fn peak_footprint_of(pid: i32) -> Option<u64> {
    // SAFETY: an all-zero struct is valid, and the call only writes that struct.
    unsafe {
        let mut info: libc::rusage_info_v4 = std::mem::zeroed();
        let status = libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, (&raw mut info).cast());
        (status == 0).then_some(info.ri_lifetime_max_phys_footprint)
    }
}

#[cfg(not(target_os = "macos"))]
fn peak_footprint_of(_pid: i32) -> Option<u64> {
    None
}

fn peak_footprint() -> Option<u64> {
    peak_footprint_of(std::process::id() as i32)
}

/// A proof is stopped before it needs more memory than this, 1.75 times the machine's. At about twice
/// the machine's, macOS ended background processes across the machine and then the proof, which left
/// no result and harmed what else was running.
fn footprint_limit() -> u64 {
    // SAFETY: neither call has preconditions.
    let (pages, page_size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    (pages.max(0) as u64 * page_size.max(0) as u64 / 4).saturating_mul(7)
}

/// Follows a child's footprint from outside, since a child the system kills cannot report it.
fn follow_footprint(pid: i32) -> Arc<AtomicU64> {
    let peak = Arc::new(AtomicU64::new(0));
    let shared = peak.clone();
    thread::spawn(move || {
        while let Some(bytes) = peak_footprint_of(pid) {
            shared.fetch_max(bytes, Ordering::Relaxed);
            thread::sleep(Duration::from_secs(1));
        }
    });
    peak
}

/// The `measure` subcommand.
pub async fn child(point_path: &Path, output: &Path) -> Result<()> {
    // SP1's logger writes to stdout, and the parent only keeps stderr.
    // SAFETY: both descriptors are open, and nothing has written to stdout yet.
    unsafe { libc::dup2(2, 1) };
    sp1_sdk::utils::setup_logger();
    record_panics();
    let point: Point = serde_json::from_slice(&fs::read(point_path)?)?;
    let shared = Arc::new(Mutex::new(Run {
        run_id: point.run_id(),
        point: Some(point.clone()),
        elf_sha256: sha256_hex(&point.arm.elf()),
        env: recorded_env(),
        ..Default::default()
    }));
    let result = measure(&point, output, &shared).await;

    // Whatever was learned before a failure is kept: a failed proof can have taken hours.
    let mut run = shared.lock().unwrap_or_else(|error| error.into_inner());
    run.peak_footprint_bytes = peak_footprint();
    match &result {
        Ok(()) => run.status = "ok".to_string(),
        Err(error) => {
            run.status = "failed".to_string();
            run.error = Some(format!("{error:#}"));
        }
    }
    fs::write(output, serde_json::to_vec(&*run)?)?;
    result
}

/// Ends the process if the proof runs too long or needs too much memory, a prover task panics, or the
/// parent is gone. Each leaves what is known so far in `output`, apart from the last, which has no
/// one to read it.
fn start_watchdog(
    output: PathBuf,
    shared: Arc<Mutex<Run>>,
    limit: Duration,
    load: LoadWatch,
    proving: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let started = Instant::now();
        // SAFETY: neither call has preconditions.
        let parent = unsafe { libc::getppid() };
        let mut panicked_at = None;
        loop {
            thread::sleep(Duration::from_secs(1));
            if unsafe { libc::getppid() } != parent {
                std::process::exit(5);
            }
            let panic = PANIC.lock().ok().and_then(|panic| panic.clone());
            if panic.is_some() && panicked_at.is_none() {
                panicked_at = Some(Instant::now());
            }
            let failed = panicked_at.is_some_and(|at| at.elapsed() > PANIC_GRACE);
            // A finished proof is a result, whatever verifying it takes.
            let proving = proving.load(Ordering::Relaxed);
            let too_large =
                proving && peak_footprint().is_some_and(|peak| peak > footprint_limit());
            let too_long = proving && started.elapsed() >= limit;
            if !failed && !too_large && !too_long {
                continue;
            }
            let mut run = shared.lock().unwrap_or_else(|error| error.into_inner());
            // The proof finished at the last moment, and its result is being written.
            if !run.status.is_empty() {
                return;
            }
            run.prove_ms = Some(started.elapsed().as_secs_f64() * 1e3);
            run.load = Some(load.sample());
            run.peak_footprint_bytes = peak_footprint();
            if failed {
                run.status = "failed".to_string();
                run.error = panic;
            } else {
                run.status = "stopped".to_string();
                if too_large {
                    run.error = Some(format!(
                        "{MEMORY_STOP} {:.0} GiB of memory",
                        footprint_limit() as f64 / (1u64 << 30) as f64
                    ));
                }
            }
            let _ = fs::write(&output, serde_json::to_vec(&*run).unwrap_or_default());
            std::process::exit(3);
        }
    });
}

async fn measure(point: &Point, output: &Path, shared: &Arc<Mutex<Run>>) -> Result<()> {
    // A stage's results are shared as soon as they exist, so that a later failure keeps them.
    let share = |run: &Run| *shared.lock().unwrap_or_else(|error| error.into_inner()) = run.clone();

    let arm = point.arm;
    let elf = arm.elf();
    let words = load_witness(point)?;
    let witness = as_bytes(&words);
    let sidecar: Sidecar = serde_json::from_slice(&fs::read(point.sidecar_path())?)?;

    // The host runs the same engine first, which gives the expected output and precompile calls.
    let started = Instant::now();
    let (expected, counts) = expected_run(arm, witness);
    let native_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut stdin = SP1Stdin::new();
    stdin.write_slice(witness);

    let light = LightProver::new().await;
    let started = Instant::now();
    let (public_values, report) = light
        .execute(elf.clone(), stdin.clone())
        .await
        .map_err(|error| anyhow!("{error}"))
        .with_context(|| format!("executing {}", point.run_id()))?;
    let execute_ms = started.elapsed().as_secs_f64() * 1e3;

    let mut run = Run {
        run_id: point.run_id(),
        point: Some(point.clone()),
        elf_sha256: sha256_hex(&elf),
        witness_sha256: sha256_hex(witness),
        env: recorded_env(),
        sidecar: Some(sidecar),
        native_ms,
        counts: Some(counts),
        cycles: report.total_instruction_count(),
        gas: report.gas(),
        execute_ms,
        opcodes: report
            .opcode_counts
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(opcode, count)| (format!("{opcode:?}"), *count))
            .collect(),
        ..Default::default()
    };
    share(&run);

    check_execution(&report, &public_values, &expected, counts)?;

    let proof_mode = match point.mode {
        RunMode::Execute => return Ok(()),
        RunMode::Core => SP1ProofMode::Core,
        RunMode::Compressed => SP1ProofMode::Compressed,
    };

    let decoy = light
        .setup(decoy_arm(arm).elf())
        .await
        .map_err(|error| anyhow!("{error}"))?;
    let decoy_vk = decoy.verifying_key().clone();
    drop(light);

    let client = ProverClient::from_env().await;
    let started = Instant::now();
    let pk = client.setup(elf).await?;
    run.setup_ms = Some(started.elapsed().as_secs_f64() * 1e3);
    run.vkey = pk.verifying_key().bytes32();
    let limit = proof_limit(run.gas, point.mode);
    run.limit_s = Some(limit.as_secs_f64());
    share(&run);

    // A proof that runs too long is recorded as such, with what is known so far.
    let monitor = LoadMonitor::start();
    let proving = Arc::new(AtomicBool::new(true));
    start_watchdog(
        output.to_path_buf(),
        shared.clone(),
        limit,
        monitor.watch(),
        proving.clone(),
    );
    let started = Instant::now();
    let result = client.prove(&pk, stdin).mode(proof_mode).await;
    proving.store(false, Ordering::Relaxed);
    run.prove_ms = Some(started.elapsed().as_secs_f64() * 1e3);
    run.load = Some(monitor.finish());
    share(&run);
    let mut proof = result.with_context(|| format!("proving {}", point.run_id()))?;

    // Recorded before verifying, so a proof that fails verification still says how large it was.
    let (shards, chip_area) = trace_details(&proof);
    run.shards = shards;
    run.trace_area = chip_area.as_ref().map(|chips| chips.values().sum());
    run.chip_area = chip_area.unwrap_or_default();
    run.proof_bytes = Some(bincode::serialized_size(&proof)?);
    share(&run);

    ensure!(
        proof.public_values.as_slice() == expected.as_bytes(),
        "the proof committed public values that differ from the host's"
    );
    let vk = pk.verifying_key();
    let mut verify = Vec::with_capacity(VERIFY_SAMPLES);
    for _ in 0..VERIFY_SAMPLES {
        let started = Instant::now();
        client.verify(&proof, vk, None).context("verifying")?;
        verify.push(started.elapsed().as_secs_f64() * 1e3);
    }
    verify.sort_by(f64::total_cmp);
    run.verify_ms = Some(verify[VERIFY_SAMPLES / 2]);
    share(&run);
    check_rejections(&client, &mut proof, vk, &decoy_vk)?;
    Ok(())
}

/// Another program's key, to show verification checks the proof.
fn decoy_arm(arm: Arm) -> Arm {
    if arm == Arm::JmtStockSha2 {
        Arm::JmtStockSha256
    } else {
        Arm::JmtStockSha2
    }
}

/// Settings that change proving, and no other variables, since those could hold keys.
fn recorded_env() -> BTreeMap<String, String> {
    const RECORDED: [&str; 10] = [
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
    std::env::vars_os()
        .filter_map(|(key, value)| {
            let key = key.into_string().ok()?;
            let recorded = RECORDED.contains(&key.as_str()) || key.starts_with("SP1_WORKER_");
            recorded.then(|| (key, value.to_string_lossy().into_owned()))
        })
        .collect()
}

/// Runs one point in a child process and appends the result. Points already recorded are skipped.
pub fn run_point(point: &Point) -> Result<Run> {
    let run_id = point.run_id();
    if let Some(done) = recorded(point)? {
        println!("{run_id}: already recorded");
        return Ok(done);
    }
    fs::create_dir_all("results")?;
    let point_path = format!("results/.point-{}.json", std::process::id());
    let output = format!("results/.run-{}.json", std::process::id());
    fs::write(&point_path, serde_json::to_vec(point)?)?;
    fs::remove_file(&output).ok();

    println!("{run_id}: running");
    let started = Instant::now();
    let mut child = Command::new(std::env::current_exe()?)
        .args(["measure", "--point", &point_path, "--output", &output])
        // Fatal errors in prover tasks only go to the log.
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "error".to_string()),
        )
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()?;
    let stderr = child.stderr.take();
    let stderr_reader = thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
        }
        text
    });
    let footprint = follow_footprint(child.id() as i32);
    let (status, peak_rss) = wait_with_rss(child.id() as i32)?;
    // The child has been reaped through `wait4`, so nothing further is waited on here.
    std::mem::forget(child);
    let stderr_text = stderr_reader.join().unwrap_or_default();

    let tail = format!(
        "{status}: {}",
        stderr_text
            .lines()
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let mut run: Run = match fs::read(&output) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(_) => Run {
            run_id: run_id.clone(),
            point: Some(point.clone()),
            status: "failed".to_string(),
            elf_sha256: sha256_hex(&point.arm.elf()),
            env: recorded_env(),
            ..Default::default()
        },
    };
    if run.status != "ok" {
        run.error = Some(match run.error.take() {
            Some(error) => format!("{error} | {tail}"),
            None => tail,
        });
    }
    run.wall_s = Some(started.elapsed().as_secs_f64());
    run.peak_rss_bytes = Some(peak_rss);
    let followed = footprint.load(Ordering::Relaxed);
    if run.peak_footprint_bytes.is_none() && followed > 0 {
        run.peak_footprint_bytes = Some(followed);
    }
    fs::remove_file(&output).ok();
    fs::remove_file(&point_path).ok();

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(RUNS_PATH)?;
    writeln!(file, "{}", serde_json::to_string(&run)?)?;

    println!(
        "{run_id}: {} in {:.0} s. native {:.1} ms, gas {:?}, instructions {}, prove {:.1} s, shards {:?}, cells {:?}, RSS {:.1} GiB{}",
        run.status,
        started.elapsed().as_secs_f64(),
        run.native_ms,
        run.gas,
        run.cycles,
        run.prove_ms.unwrap_or_default() / 1e3,
        run.shards,
        run.trace_area,
        peak_rss as f64 / (1u64 << 30) as f64,
        run.error
            .as_deref()
            .map_or(String::new(), |error| format!("\n  {error}"))
    );
    Ok(run)
}

pub fn is_recorded(point: &Point) -> bool {
    matches!(recorded(point), Ok(Some(_)))
}

fn recorded(point: &Point) -> Result<Option<Run>> {
    let Ok(text) = fs::read_to_string(RUNS_PATH) else {
        return Ok(None);
    };
    // A missing witness has to be rebuilt before anything can be said about it.
    let Ok(words) = load_witness(point) else {
        return Ok(None);
    };
    let witness_sha256 = sha256_hex(as_bytes(&words));
    let (run_id, elf_sha256, env) = (point.run_id(), sha256_hex(&point.arm.elf()), recorded_env());
    for line in text.lines() {
        let run: Run = serde_json::from_str(line)?;
        // A failed run is tried again. A proof stopped for time or memory is a result until a longer
        // limit of that kind applies. A run of another build or witness, or a proof under other
        // settings, says nothing about this one.
        let done = match run.status.as_str() {
            "ok" => true,
            "stopped" if run.stopped_for_memory() => run
                .peak_footprint_bytes
                .is_some_and(|peak| peak > footprint_limit()),
            "stopped" => run
                .limit_s
                .is_some_and(|limit| limit >= proof_limit(run.gas, point.mode).as_secs_f64()),
            _ => false,
        };
        if done
            && run.run_id == run_id
            && run.elf_sha256 == elf_sha256
            && run.witness_sha256 == witness_sha256
            && (point.mode == RunMode::Execute || run.env == env)
        {
            return Ok(Some(run));
        }
    }
    Ok(None)
}

/// The latest run of each point under each prover setting, leaving out older builds of the guests.
pub fn current_runs() -> Result<Vec<Run>> {
    let text = fs::read_to_string(RUNS_PATH)?;
    let mut elves = HashMap::new();
    let mut runs: Vec<Run> = Vec::new();
    for line in text.lines() {
        let run: Run = serde_json::from_str(line)?;
        let Some(point) = &run.point else { continue };
        let arm = point.arm;
        let current = elves.entry(arm).or_insert_with(|| sha256_hex(&arm.elf()));
        if run.elf_sha256 == *current {
            runs.retain(|earlier| earlier.run_id != run.run_id || earlier.env != run.env);
            runs.push(run);
        }
    }
    Ok(runs)
}

/// Waits for a child and returns its status and peak resident memory.
fn wait_with_rss(pid: i32) -> Result<(ExitStatus, u64)> {
    let mut status = 0;
    // SAFETY: an all-zero `rusage` is valid, and both pointers outlive the call.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
        if result == pid {
            break;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error.into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    // On macOS `ru_maxrss` is in bytes.
    Ok((ExitStatus::from_raw(status), usage.ru_maxrss as u64))
}

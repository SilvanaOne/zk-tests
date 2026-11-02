//! Proof checks and what the machine was doing during a proof.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use slop_algebra::PrimeField32;
use sp1_sdk::{Prover, SP1Proof, SP1ProofWithPublicValues, SP1PublicValues, SP1VerifyingKey};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// How often to sample CPU use by other processes during a proof.
const LOAD_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Requires verification to fail for changed public values and for another program's key. Public
/// values are checked before the proof itself, so the key is what exercises the proof check.
pub fn check_rejections(
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
/// counts real rows, so unlike gas it includes all memory and precompile traffic.
pub fn trace_details(
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

/// Conditions during one proof that could disturb its timing.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LoadSample {
    /// Mean CPU used by other processes during the proof, in cores, or None if it was not sampled.
    pub other_cpu_cores: Option<f64>,
    pub swap_grew_bytes: u64,
    pub swap_used_bytes: u64,
}

/// Samples CPU use by everything except this process while a proof runs.
pub struct LoadMonitor {
    stop: Arc<AtomicBool>,
    sampler: thread::JoinHandle<()>,
    watch: LoadWatch,
}

/// Reads what a monitor has seen so far, for a proof that never finishes.
#[derive(Clone)]
pub struct LoadWatch {
    /// The sum of other processes' CPU use over all samples, in cores, and the number of samples.
    totals: Arc<Mutex<(f64, u32)>>,
    swap_before: u64,
}

impl LoadWatch {
    pub fn sample(&self) -> LoadSample {
        let swap = used_swap();
        let (total, samples) = *self
            .totals
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        LoadSample {
            other_cpu_cores: (samples > 0).then(|| total / samples as f64),
            swap_grew_bytes: swap.saturating_sub(self.swap_before),
            swap_used_bytes: swap,
        }
    }
}

impl LoadMonitor {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let totals = Arc::new(Mutex::new((0.0, 0)));
        let sampler = {
            let (stop, totals) = (stop.clone(), totals.clone());
            thread::spawn(move || sample_other_cpu(&stop, &totals))
        };
        Self {
            stop,
            sampler,
            watch: LoadWatch {
                totals,
                swap_before: used_swap(),
            },
        }
    }

    pub fn watch(&self) -> LoadWatch {
        self.watch.clone()
    }

    pub fn finish(self) -> LoadSample {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.sampler.join();
        self.watch.sample()
    }
}

fn used_swap() -> u64 {
    let Ok(mut system) = System::new() else {
        return 0;
    };
    system.refresh_memory();
    system.used_swap()
}

fn sample_other_cpu(stop: &AtomicBool, totals: &Mutex<(f64, u32)>) {
    let Ok(pid) = sysinfo::get_current_pid() else {
        return;
    };
    let Ok(mut system) = System::new() else {
        return;
    };
    refresh_cpu(&mut system, pid);

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
        let mut totals = totals.lock().unwrap_or_else(|error| error.into_inner());
        totals.0 += (all - own).max(0.0);
        totals.1 += 1;
    }
}

fn refresh_cpu(system: &mut System, pid: Pid) {
    system.refresh_cpu_usage();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );
}

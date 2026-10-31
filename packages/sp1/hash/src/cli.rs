//! Command line options.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::Serialize;
use sp1_sdk::SP1ProofMode;

use crate::variant::Variant;

/// The proof modes to measure. The Groth16 and PLONK wrap stages cost the same for every hash, so
/// they cannot change the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Serialize)]
#[clap(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// STARK proof per shard, growing with the workload.
    Core,
    /// Recursively compressed into one constant-size proof.
    Compressed,
}

impl Mode {
    pub fn proof_mode(self) -> SP1ProofMode {
        match self {
            Self::Core => SP1ProofMode::Core,
            Self::Compressed => SP1ProofMode::Compressed,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Compressed => "compressed",
        }
    }
}

/// Measures SHA-256, Keccak-256 and Poseidon2 natively and inside SP1: x hashes of x random data
/// sets, proved and verified. The prover comes from SP1_PROVER (cpu by default, or mock).
#[derive(Debug, Parser)]
#[command(name = "hash-bench", version)]
pub struct Args {
    /// Bytes per data set, comma separated. Accepts K and M suffixes.
    #[arg(short = 's', long = "data-size", value_delimiter = ',', default_value = "64", value_parser = parse_size)]
    pub data_sizes: Vec<usize>,

    /// Number of data sets x, which is also the number of hashes per proof, comma separated.
    #[arg(short = 'n', long = "count", value_delimiter = ',', default_value = "1000", value_parser = parse_size)]
    pub counts: Vec<usize>,

    /// Variants to run, comma separated. Defaults to all that apply. Without baseline there are
    /// no net figures.
    #[arg(short = 'v', long = "variants", value_delimiter = ',', value_enum)]
    pub variants: Vec<Variant>,

    /// Proof modes, comma separated.
    #[arg(short = 'm', long = "modes", value_delimiter = ',', value_enum, default_values = ["core", "compressed"])]
    pub modes: Vec<Mode>,

    /// Proofs per variant and mode. The first is reported as cold and left out of rankings when
    /// there are more. Prove time is only ranked with at least 3 warm proofs, so use 4 or more.
    #[arg(short = 'r', long = "runs", default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub runs: u32,

    /// Report native timing, cycles, gas and syscalls only, without proving.
    #[arg(long)]
    pub execute_only: bool,

    /// Seed for the random data and the run order. Random by default, and always printed.
    #[arg(long)]
    pub seed: Option<u64>,

    /// Write the report to this JSON file, updated after every measurement.
    #[arg(long)]
    pub json: Option<PathBuf>,
}

/// The resolved options for one benchmark run.
#[derive(Debug)]
pub struct Config {
    pub data_sizes: Vec<usize>,
    pub counts: Vec<usize>,
    pub variants: Vec<Variant>,
    pub modes: Vec<Mode>,
    pub runs: usize,
    pub execute_only: bool,
    pub seed: u64,
    pub json: Option<PathBuf>,
}

impl Args {
    pub fn into_config(self) -> Config {
        Config {
            data_sizes: dedup(self.data_sizes),
            counts: dedup(self.counts),
            variants: if self.variants.is_empty() {
                Variant::ALL.to_vec()
            } else {
                dedup(self.variants)
            },
            modes: dedup(self.modes),
            runs: self.runs as usize,
            execute_only: self.execute_only,
            seed: self.seed.unwrap_or_else(rand::random),
            json: self.json,
        }
    }
}

/// Removes repeats, keeping first occurrences in the given order.
fn dedup<T: PartialEq>(values: Vec<T>) -> Vec<T> {
    let mut unique = Vec::with_capacity(values.len());
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

/// Parses a positive number with an optional binary K or M suffix.
fn parse_size(text: &str) -> Result<usize, String> {
    let text = text.trim();
    let (digits, unit) = match text.as_bytes().last() {
        Some(b'k' | b'K') => (&text[..text.len() - 1], 1 << 10),
        Some(b'm' | b'M') => (&text[..text.len() - 1], 1 << 20),
        _ => (text, 1),
    };
    let value: usize = digits
        .parse()
        .map_err(|_| format!("`{text}` is not a number with an optional K or M suffix"))?;
    if value == 0 {
        return Err("must be at least 1".to_string());
    }
    value
        .checked_mul(unit)
        .ok_or_else(|| format!("`{text}` is too large"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("64"), Ok(64));
        assert_eq!(parse_size("1K"), Ok(1024));
        assert_eq!(parse_size("2k"), Ok(2048));
        assert_eq!(parse_size("1M"), Ok(1 << 20));
        assert!(parse_size("0").is_err());
        assert!(parse_size("0K").is_err());
        assert!(parse_size("").is_err());
        assert!(parse_size("K").is_err());
        assert!(parse_size("1G").is_err());
        assert!(parse_size("-1").is_err());
    }

    #[test]
    fn parses_lists_and_defaults() {
        let config =
            Args::parse_from(["hash-bench", "-s", "1K,40,96", "-n", "10,20"]).into_config();
        assert_eq!(config.data_sizes, [1024, 40, 96]);
        assert_eq!(config.counts, [10, 20]);
        assert_eq!(config.variants, Variant::ALL);
        assert_eq!(config.modes, [Mode::Core, Mode::Compressed]);
        assert_eq!(config.runs, 1);

        let config = Args::parse_from([
            "hash-bench",
            "-v",
            "baseline,sha256_node",
            "-m",
            "core",
            "--seed",
            "7",
        ])
        .into_config();
        assert_eq!(config.data_sizes, [64]);
        assert_eq!(config.counts, [1000]);
        assert_eq!(config.variants, [Variant::Baseline, Variant::Sha256Node]);
        assert_eq!(config.modes, [Mode::Core]);
        assert_eq!(config.seed, 7);
    }

    #[test]
    fn rejects_zero_runs() {
        assert!(Args::try_parse_from(["hash-bench", "-r", "0"]).is_err());
    }
}

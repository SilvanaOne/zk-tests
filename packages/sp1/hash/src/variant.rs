//! The benchmark variants: which guest runs, and which precompile calls it must make.

use clap::ValueEnum;
use serde::Serialize;
use sp1_core_executor::SyscallCode;
use sp1_sdk::{Elf, ExecutionReport, include_elf};

/// The size of a Merkle node preimage: two 32-byte children.
pub const NODE_SIZE: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum, Serialize)]
#[clap(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    /// No hash: reads every input word, so subtracting it removes input handling.
    Baseline,
    /// SHA-256 of any size through the patched sha2 crate.
    Sha256,
    /// Keccak-256 of any size through the patched tiny-keccak crate.
    Keccak256,
    /// Poseidon2 of any size through SP1's byte sponge.
    Poseidon2,
    /// SHA-256 of a 64-byte node with raw precompile calls on native words.
    Sha256Node,
    /// Keccak-256 of a 64-byte node with one raw precompile call on native lanes.
    Keccak256Node,
    /// Poseidon2 of a 64-byte node with one precompile call: Trunc8(P(x)).
    Poseidon2Node,
    /// Poseidon2 of a 64-byte node with one precompile call and a feed-forward: Trunc8(P(x) + x).
    Poseidon2NodeFf,
}

/// Results are only comparable within a tier and data size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Control,
    Bytes,
    Node,
}

impl Variant {
    pub const ALL: [Self; 8] = [
        Self::Baseline,
        Self::Sha256,
        Self::Keccak256,
        Self::Poseidon2,
        Self::Sha256Node,
        Self::Keccak256Node,
        Self::Poseidon2Node,
        Self::Poseidon2NodeFf,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Sha256 => "sha256",
            Self::Keccak256 => "keccak256",
            Self::Poseidon2 => "poseidon2",
            Self::Sha256Node => "sha256_node",
            Self::Keccak256Node => "keccak256_node",
            Self::Poseidon2Node => "poseidon2_node",
            Self::Poseidon2NodeFf => "poseidon2_node_ff",
        }
    }

    pub fn elf(self) -> Elf {
        match self {
            Self::Baseline => include_elf!("baseline"),
            Self::Sha256 => include_elf!("sha256"),
            Self::Keccak256 => include_elf!("keccak256"),
            Self::Poseidon2 => include_elf!("poseidon2"),
            Self::Sha256Node => include_elf!("sha256_node"),
            Self::Keccak256Node => include_elf!("keccak256_node"),
            Self::Poseidon2Node => include_elf!("poseidon2_node"),
            Self::Poseidon2NodeFf => include_elf!("poseidon2_node_ff"),
        }
    }

    pub fn tier(self) -> Tier {
        match self {
            Self::Baseline => Tier::Control,
            Self::Sha256 | Self::Keccak256 | Self::Poseidon2 => Tier::Bytes,
            Self::Sha256Node
            | Self::Keccak256Node
            | Self::Poseidon2Node
            | Self::Poseidon2NodeFf => Tier::Node,
        }
    }

    /// Node variants only accept a 64-byte preimage.
    pub fn runs_at(self, data_size: usize) -> bool {
        self.tier() != Tier::Node || data_size == NODE_SIZE
    }

    /// The Poseidon2 node variants take canonical field elements rather than arbitrary bytes.
    pub fn takes_field_elements(self) -> bool {
        matches!(self, Self::Poseidon2Node | Self::Poseidon2NodeFf)
    }

    /// The hash precompile calls a guest must make. Anything else means a precompile patch was
    /// dropped or a guest changed, so the numbers would not measure what they claim to.
    pub fn expected_syscalls(self, data_size: usize, count: usize) -> SyscallCounts {
        let (n, x) = (data_size as u64, count as u64);
        // Every guest hashes its 48 bytes of public values, which is one SHA-256 block.
        let mut calls = SyscallCounts {
            sha_extend: 1,
            sha_compress: 1,
            ..Default::default()
        };
        match self {
            Self::Baseline => {}
            Self::Sha256 => {
                let blocks = (n + 8) / 64 + 1;
                calls.sha_extend += x * blocks;
                calls.sha_compress += x * blocks;
            }
            Self::Keccak256 => calls.keccak_permute += x * (n / 136 + 1),
            Self::Poseidon2 => calls.poseidon2 += x * (1 + n.div_ceil(24)),
            // The padding block's schedule is computed once, then each node extends one block and
            // compresses two.
            Self::Sha256Node => {
                calls.sha_extend += 1 + x;
                calls.sha_compress += 2 * x;
            }
            Self::Keccak256Node => calls.keccak_permute += x,
            Self::Poseidon2Node | Self::Poseidon2NodeFf => calls.poseidon2 += x,
        }
        calls
    }
}

/// Invocation counts of the hash precompiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SyscallCounts {
    pub sha_extend: u64,
    pub sha_compress: u64,
    pub keccak_permute: u64,
    pub poseidon2: u64,
}

impl SyscallCounts {
    pub fn from_report(report: &ExecutionReport) -> Self {
        Self {
            sha_extend: report.syscall_counts[SyscallCode::SHA_EXTEND],
            sha_compress: report.syscall_counts[SyscallCode::SHA_COMPRESS],
            keccak_permute: report.syscall_counts[SyscallCode::KECCAK_PERMUTE],
            poseidon2: report.syscall_counts[SyscallCode::POSEIDON2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_cli() {
        for variant in Variant::ALL {
            let value = variant.to_possible_value().unwrap();
            assert_eq!(value.get_name(), variant.name());
        }
    }

    #[test]
    fn sha256_counts_blocks_including_padding() {
        for (size, blocks) in [(1, 1), (55, 1), (56, 2), (64, 2), (119, 2), (120, 3)] {
            let calls = Variant::Sha256.expected_syscalls(size, 10);
            assert_eq!(calls.sha_extend, 1 + 10 * blocks, "size {size}");
            assert_eq!(calls.sha_compress, 1 + 10 * blocks, "size {size}");
            assert_eq!(calls.keccak_permute, 0);
            assert_eq!(calls.poseidon2, 0);
        }
    }

    #[test]
    fn keccak256_counts_permutations_at_the_rate_boundary() {
        for (size, permutations) in [(1, 1), (64, 1), (135, 1), (136, 2), (137, 2), (272, 3)] {
            let calls = Variant::Keccak256.expected_syscalls(size, 10);
            assert_eq!(calls.keccak_permute, 10 * permutations, "size {size}");
            assert_eq!(calls.sha_extend, 1);
            assert_eq!(calls.sha_compress, 1);
        }
    }

    #[test]
    fn poseidon2_counts_the_length_block_and_data_blocks() {
        for (size, permutations) in [(1, 2), (24, 2), (25, 3), (64, 4), (96, 5)] {
            let calls = Variant::Poseidon2.expected_syscalls(size, 10);
            assert_eq!(calls.poseidon2, 10 * permutations, "size {size}");
        }
    }

    #[test]
    fn node_variants_only_run_at_64_bytes_and_count_their_calls() {
        assert!(Variant::Sha256Node.runs_at(64));
        assert!(!Variant::Sha256Node.runs_at(96));
        assert!(Variant::Sha256.runs_at(96));
        assert!(Variant::Baseline.runs_at(96));

        let sha = Variant::Sha256Node.expected_syscalls(64, 10);
        assert_eq!((sha.sha_extend, sha.sha_compress), (1 + 1 + 10, 1 + 20));
        assert_eq!(
            Variant::Keccak256Node
                .expected_syscalls(64, 10)
                .keccak_permute,
            10
        );
        assert_eq!(
            Variant::Poseidon2Node.expected_syscalls(64, 10).poseidon2,
            10
        );
        assert_eq!(
            Variant::Poseidon2NodeFf.expected_syscalls(64, 10).poseidon2,
            10
        );
    }

    #[test]
    fn baseline_only_hashes_its_public_values() {
        let calls = Variant::Baseline.expected_syscalls(64, 10);
        assert_eq!(
            calls,
            SyscallCounts {
                sha_extend: 1,
                sha_compress: 1,
                keccak_permute: 0,
                poseidon2: 0
            }
        );
    }
}

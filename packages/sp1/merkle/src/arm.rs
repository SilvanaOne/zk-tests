//! The guests, and how the host runs the same engine.

use clap::ValueEnum;
use merkle_map::{
    hash::{Poseidon2, Sha2Crate, Sha256Raw},
    imm,
    jellyfish::{
        self,
        hashers::{PoseidonNode, PoseidonSponge, Sha2Counted, Sha256Stream},
        stock,
    },
    record::PublicValues,
};
use serde::{Deserialize, Serialize};
use sp1_sdk::{Elf, include_elf};

use crate::software::Native;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum)]
#[clap(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Arm {
    ImmChainedSha2,
    ImmChainedSha256,
    ImmChainedPoseidon2,
    ImmBatchSha256,
    ImmBatchPoseidon2,
    JmtStockSha2,
    JmtStockSha256,
    JmtStockPoseidon2Sponge,
    JmtStockPoseidon2Node,
    JmtBatchSha256,
    JmtBatchPoseidon2,
}

/// Arms in one group have the same tree, so one built state serves all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Group {
    ImmSha256,
    ImmPoseidon2,
    JmtSha256,
    JmtPoseidon2Sponge,
    JmtPoseidon2Node,
}

/// What a witness looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// One proof per record.
    Chained,
    /// The unchanged `jmt` crate's proofs.
    Stock,
    /// One deduplicated witness per batch.
    Batch,
}

impl Arm {
    pub const ALL: [Self; 11] = [
        Self::ImmChainedSha2,
        Self::ImmChainedSha256,
        Self::ImmChainedPoseidon2,
        Self::ImmBatchSha256,
        Self::ImmBatchPoseidon2,
        Self::JmtStockSha2,
        Self::JmtStockSha256,
        Self::JmtStockPoseidon2Sponge,
        Self::JmtStockPoseidon2Node,
        Self::JmtBatchSha256,
        Self::JmtBatchPoseidon2,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ImmChainedSha2 => "imm_chained_sha2",
            Self::ImmChainedSha256 => "imm_chained_sha256",
            Self::ImmChainedPoseidon2 => "imm_chained_poseidon2",
            Self::ImmBatchSha256 => "imm_batch_sha256",
            Self::ImmBatchPoseidon2 => "imm_batch_poseidon2",
            Self::JmtStockSha2 => "jmt_stock_sha2",
            Self::JmtStockSha256 => "jmt_stock_sha256",
            Self::JmtStockPoseidon2Sponge => "jmt_stock_poseidon2_sponge",
            Self::JmtStockPoseidon2Node => "jmt_stock_poseidon2_node",
            Self::JmtBatchSha256 => "jmt_batch_sha256",
            Self::JmtBatchPoseidon2 => "jmt_batch_poseidon2",
        }
    }

    pub fn elf(self) -> Elf {
        match self {
            Self::ImmChainedSha2 => include_elf!("imm_chained_sha2"),
            Self::ImmChainedSha256 => include_elf!("imm_chained_sha256"),
            Self::ImmChainedPoseidon2 => include_elf!("imm_chained_poseidon2"),
            Self::ImmBatchSha256 => include_elf!("imm_batch_sha256"),
            Self::ImmBatchPoseidon2 => include_elf!("imm_batch_poseidon2"),
            Self::JmtStockSha2 => include_elf!("jmt_stock_sha2"),
            Self::JmtStockSha256 => include_elf!("jmt_stock_sha256"),
            Self::JmtStockPoseidon2Sponge => include_elf!("jmt_stock_poseidon2_sponge"),
            Self::JmtStockPoseidon2Node => include_elf!("jmt_stock_poseidon2_node"),
            Self::JmtBatchSha256 => include_elf!("jmt_batch_sha256"),
            Self::JmtBatchPoseidon2 => include_elf!("jmt_batch_poseidon2"),
        }
    }

    pub fn group(self) -> Group {
        match self {
            Self::ImmChainedSha2 | Self::ImmChainedSha256 | Self::ImmBatchSha256 => {
                Group::ImmSha256
            }
            Self::ImmChainedPoseidon2 | Self::ImmBatchPoseidon2 => Group::ImmPoseidon2,
            Self::JmtStockSha2 | Self::JmtStockSha256 | Self::JmtBatchSha256 => Group::JmtSha256,
            Self::JmtStockPoseidon2Sponge => Group::JmtPoseidon2Sponge,
            Self::JmtStockPoseidon2Node | Self::JmtBatchPoseidon2 => Group::JmtPoseidon2Node,
        }
    }

    pub fn engine(self) -> Engine {
        match self {
            Self::ImmChainedSha2 | Self::ImmChainedSha256 | Self::ImmChainedPoseidon2 => {
                Engine::Chained
            }
            Self::ImmBatchSha256
            | Self::ImmBatchPoseidon2
            | Self::JmtBatchSha256
            | Self::JmtBatchPoseidon2 => Engine::Batch,
            Self::JmtStockSha2
            | Self::JmtStockSha256
            | Self::JmtStockPoseidon2Sponge
            | Self::JmtStockPoseidon2Node => Engine::Stock,
        }
    }

    pub fn is_jmt(self) -> bool {
        matches!(
            self.group(),
            Group::JmtSha256 | Group::JmtPoseidon2Sponge | Group::JmtPoseidon2Node
        )
    }

    /// Runs the guest's engine on the host, counting the precompile calls the guest must make.
    pub fn native(self, witness: &[u8]) -> PublicValues {
        type Sha = Sha256Raw<Native>;
        type Crate = Sha2Crate<Native>;
        type Pos = Poseidon2<Native>;
        match self {
            Self::ImmChainedSha2 => imm::chained::run::<Crate>(witness),
            Self::ImmChainedSha256 => imm::chained::run::<Sha>(witness),
            Self::ImmChainedPoseidon2 => imm::chained::run::<Pos>(witness),
            Self::ImmBatchSha256 => imm::batch::run::<Sha>(witness),
            Self::ImmBatchPoseidon2 => imm::batch::run::<Pos>(witness),
            Self::JmtStockSha2 => stock::run::<Crate, Sha2Counted<Native>>(witness),
            Self::JmtStockSha256 => stock::run::<Sha, Sha256Stream<Native>>(witness),
            Self::JmtStockPoseidon2Sponge => stock::run::<Pos, PoseidonSponge<Native>>(witness),
            Self::JmtStockPoseidon2Node => stock::run::<Pos, PoseidonNode<Native>>(witness),
            Self::JmtBatchSha256 => jellyfish::batch::run::<Sha>(witness),
            Self::JmtBatchPoseidon2 => jellyfish::batch::run::<Pos>(witness),
        }
    }
}

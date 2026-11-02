//! Leaves, proofs and witnesses. See the module notice in `imm` for the origin of this code.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::{Digest, ImmHasher};

/// A leaf of the sorted linked list the tree commits to. Its position is the path to it, so it does
/// not hold its index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Leaf {
    pub key: Digest,
    pub value: Digest,
    pub next_key: Digest,
}

impl Leaf {
    /// The sentinel at index 0, whose zero key is below every other key.
    pub const fn empty() -> Self {
        Self {
            key: Digest::ZERO,
            value: Digest::ZERO,
            next_key: Digest::ZERO,
        }
    }

    pub fn hash<H: ImmHasher>(&self) -> Digest {
        H::leaf(&self.key, &self.value, &self.next_key)
    }
}

/// The siblings from a leaf up to the internal root, lowest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct MerkleProof {
    pub siblings: Vec<Digest>,
}

/// A leaf and where it sits.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct MembershipProof {
    pub leaf: Leaf,
    pub index: u64,
    pub merkle_proof: MerkleProof,
}

/// The leaf whose key is the largest below an absent key.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct NonMembershipProof {
    pub low_leaf: Leaf,
    pub index: u64,
    pub merkle_proof: MerkleProof,
}

/// What an update needs beyond the key and values a guest derives.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct UpdateWitness {
    pub membership_proof: MembershipProof,
}

/// What an insert needs. The slot's siblings are read after the low leaf changes, since the two
/// paths can share upper levels.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InsertWitness {
    pub non_membership_proof: NonMembershipProof,
    pub slot_proof: MerkleProof,
}

/// One record's proof in a chained batch.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum RecordWitness {
    Insert(InsertWitness),
    Update(UpdateWitness),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexedMerkleMapError {
    KeyAlreadyExists,
    KeyDoesNotExist,
    TreeFull,
    InvalidProof,
    InvalidWitness,
    InvalidLeafIndex,
    RootMismatch,
}

impl core::fmt::Display for IndexedMerkleMapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl core::error::Error for IndexedMerkleMapError {}

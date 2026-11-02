//! Operations that work from witnesses alone, one record at a time, for use inside a zkVM. See the
//! module notice in `imm` for the origin of this code.

use core::marker::PhantomData;

use crate::{
    hash::{Digest, ImmHasher},
    imm::types::{
        IndexedMerkleMapError, InsertWitness, Leaf, MembershipProof, MerkleProof,
        NonMembershipProof, UpdateWitness,
    },
};

pub struct ProvableIndexedMerkleMap<H>(PhantomData<H>);

impl<H: ImmHasher> ProvableIndexedMerkleMap<H> {
    /// Hashes a leaf up to the internal root. Each bit of the index says which side it is on.
    #[inline(always)]
    pub fn compute_root(leaf_hash: Digest, index: u64, proof: &MerkleProof) -> Digest {
        let mut current = leaf_hash;
        for (level, sibling) in proof.siblings.iter().enumerate() {
            current = if (index >> level) & 1 == 1 {
                H::node(sibling, &current)
            } else {
                H::node(&current, sibling)
            };
        }
        current
    }

    /// The same, where a subtree with nothing in it stays `EMPTY` without hashing.
    #[inline(always)]
    fn compute_root_or_empty(start: Digest, index: u64, proof: &MerkleProof) -> Digest {
        let mut current = start;
        for (level, sibling) in proof.siblings.iter().enumerate() {
            current = if (index >> level) & 1 == 1 {
                H::node_or_empty(sibling, &current)
            } else {
                H::node_or_empty(&current, sibling)
            };
        }
        current
    }

    /// Checks that a key holds a value under the internal root.
    pub fn verify_membership_proof(
        internal_root: &Digest,
        proof: &MembershipProof,
        key: &Digest,
        value: &Digest,
        depth: usize,
    ) -> bool {
        proof.leaf.key == *key
            && proof.leaf.value == *value
            && proof.merkle_proof.siblings.len() == depth
            && Self::compute_root(proof.leaf.hash::<H>(), proof.index, &proof.merkle_proof)
                == *internal_root
    }

    /// Checks that a key is absent: a leaf in the tree has the largest key below it, and that leaf's
    /// next key is above it or the end of the list.
    pub fn verify_non_membership_proof(
        internal_root: &Digest,
        proof: &NonMembershipProof,
        key: &Digest,
        depth: usize,
    ) -> bool {
        let low = &proof.low_leaf;
        *key > low.key
            && (low.next_key == Digest::ZERO || *key < low.next_key)
            && proof.merkle_proof.siblings.len() == depth
            && Self::compute_root(low.hash::<H>(), proof.index, &proof.merkle_proof)
                == *internal_root
    }

    /// Verifies an update and returns the new internal root.
    pub fn update(
        internal_root: &Digest,
        witness: &UpdateWitness,
        key: &Digest,
        old_value: &Digest,
        new_value: &Digest,
        depth: usize,
    ) -> Result<Digest, IndexedMerkleMapError> {
        let proof = &witness.membership_proof;
        if !Self::verify_membership_proof(internal_root, proof, key, old_value, depth) {
            return Err(IndexedMerkleMapError::InvalidProof);
        }
        let updated = Leaf {
            value: *new_value,
            ..proof.leaf
        };
        Ok(Self::compute_root(
            updated.hash::<H>(),
            proof.index,
            &proof.merkle_proof,
        ))
    }

    /// Verifies an insert at slot `length` and returns the new internal root. The caller must have
    /// checked `length` against the root it is bound into.
    ///
    /// Each step is tied to the last: the low leaf is in the tree, updating it gives an intermediate
    /// root, the slot is empty under that root, and the new leaf goes on the slot's own siblings.
    pub fn insert(
        internal_root: &Digest,
        length: u64,
        witness: &InsertWitness,
        key: &Digest,
        value: &Digest,
        depth: usize,
    ) -> Result<Digest, IndexedMerkleMapError> {
        if depth >= 64 || length >= 1u64 << depth {
            return Err(IndexedMerkleMapError::TreeFull);
        }
        let proof = &witness.non_membership_proof;
        if !Self::verify_non_membership_proof(internal_root, proof, key, depth) {
            return Err(IndexedMerkleMapError::InvalidProof);
        }
        if witness.slot_proof.siblings.len() != depth {
            return Err(IndexedMerkleMapError::InvalidWitness);
        }

        let updated_low = Leaf {
            next_key: *key,
            ..proof.low_leaf
        };
        let after_low =
            Self::compute_root(updated_low.hash::<H>(), proof.index, &proof.merkle_proof);

        if Self::compute_root_or_empty(H::EMPTY, length, &witness.slot_proof) != after_low {
            return Err(IndexedMerkleMapError::RootMismatch);
        }

        let new_leaf = Leaf {
            key: *key,
            value: *value,
            next_key: proof.low_leaf.next_key,
        };
        Ok(Self::compute_root_or_empty(
            new_leaf.hash::<H>(),
            length,
            &witness.slot_proof,
        ))
    }
}

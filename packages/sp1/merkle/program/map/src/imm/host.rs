//! The full map, for use outside a zkVM. See the module notice in `imm` for the origin of this code.

use std::{collections::BTreeMap, marker::PhantomData};

use crate::{
    hash::{Digest, ImmHasher},
    imm::types::{
        IndexedMerkleMapError, InsertWitness, Leaf, MembershipProof, MerkleProof,
        NonMembershipProof, UpdateWitness,
    },
};

/// The sorted linked list of leaves and the tree over them.
///
/// Leaves fill the tree left to right, so each level is a `Vec` and anything past its end is `EMPTY`.
pub struct IndexedMerkleMap<H> {
    height: usize,
    leaves: Vec<Leaf>,
    order: BTreeMap<Digest, usize>,
    levels: Vec<Vec<Digest>>,
    _hasher: PhantomData<H>,
}

impl<H> Clone for IndexedMerkleMap<H> {
    fn clone(&self) -> Self {
        Self {
            height: self.height,
            leaves: self.leaves.clone(),
            order: self.order.clone(),
            levels: self.levels.clone(),
            _hasher: PhantomData,
        }
    }
}

impl<H: ImmHasher> IndexedMerkleMap<H> {
    /// A map holding only the sentinel leaf. It has `height - 1` levels of siblings, so it holds
    /// `2^(height - 1)` leaves including the sentinel.
    pub fn new(height: usize) -> Self {
        Self::bulk_load(height, core::iter::empty())
    }

    /// A map holding these records, in this leaf order after the sentinel. Hashes each node once.
    ///
    /// # Panics
    /// If a key repeats, a key is zero, or the records do not fit.
    pub fn bulk_load(height: usize, records: impl Iterator<Item = (Digest, Digest)>) -> Self {
        assert!(
            (1..=63).contains(&height),
            "height must be between 1 and 63"
        );
        let depth = height - 1;

        let mut leaves = vec![Leaf::empty()];
        leaves.extend(records.map(|(key, value)| Leaf {
            key,
            value,
            next_key: Digest::ZERO,
        }));
        assert!(leaves.len() as u64 <= 1u64 << depth, "too many records");

        let mut sorted: Vec<(Digest, usize)> = leaves
            .iter()
            .enumerate()
            .map(|(index, leaf)| (leaf.key, index))
            .collect();
        sorted.sort_unstable();
        for pair in sorted.windows(2) {
            assert!(pair[0].0 != pair[1].0, "repeated key");
            leaves[pair[0].1].next_key = pair[1].0;
        }

        let mut levels = Vec::with_capacity(depth + 1);
        levels.push(leaves.iter().map(Leaf::hash::<H>).collect::<Vec<_>>());
        for level in 0..depth {
            let parents = levels[level]
                .chunks(2)
                .map(|pair: &[Digest]| H::node_or_empty(&pair[0], pair.get(1).unwrap_or(&H::EMPTY)))
                .collect();
            levels.push(parents);
        }
        Self {
            height,
            leaves,
            order: sorted.into_iter().collect(),
            levels,
            _hasher: PhantomData,
        }
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The number of siblings on a path.
    pub fn depth(&self) -> usize {
        self.height - 1
    }

    /// The number of leaves, including the sentinel.
    pub fn length(&self) -> u64 {
        self.leaves.len() as u64
    }

    pub fn internal_root(&self) -> Digest {
        self.levels[self.depth()][0]
    }

    /// The root with the length bound in.
    pub fn root(&self) -> Digest {
        H::bind(&self.internal_root(), self.length())
    }

    /// The node at a level, or `EMPTY` past the filled part.
    pub fn node(&self, level: usize, index: u64) -> Digest {
        self.levels[level]
            .get(index as usize)
            .copied()
            .unwrap_or(H::EMPTY)
    }

    pub fn leaf(&self, index: u64) -> &Leaf {
        &self.leaves[index as usize]
    }

    pub fn index_of(&self, key: &Digest) -> Option<u64> {
        self.order.get(key).map(|index| *index as u64)
    }

    /// The index of the leaf with the largest key below this one.
    pub fn low_index(&self, key: &Digest) -> u64 {
        // The sentinel's zero key is below every other key, so only a zero key finds nothing.
        self.order
            .range(..*key)
            .next_back()
            .map_or(0, |(_, index)| *index as u64)
    }

    pub fn merkle_proof(&self, index: u64) -> MerkleProof {
        MerkleProof {
            siblings: (0..self.depth())
                .map(|level| self.node(level, (index >> level) ^ 1))
                .collect(),
        }
    }

    pub fn get_option(&self, key: &Digest) -> Option<Digest> {
        self.index_of(key).map(|index| self.leaf(index).value)
    }

    pub fn get_membership_proof(&self, key: &Digest) -> Option<MembershipProof> {
        let index = self.index_of(key)?;
        Some(MembershipProof {
            leaf: *self.leaf(index),
            index,
            merkle_proof: self.merkle_proof(index),
        })
    }

    pub fn get_non_membership_proof(&self, key: &Digest) -> Option<NonMembershipProof> {
        if self.order.contains_key(key) {
            return None;
        }
        let index = self.low_index(key);
        Some(NonMembershipProof {
            low_leaf: *self.leaf(index),
            index,
            merkle_proof: self.merkle_proof(index),
        })
    }

    fn write_leaf(&mut self, index: usize, leaf: Leaf) {
        let hash = leaf.hash::<H>();
        if index == self.leaves.len() {
            self.leaves.push(leaf);
        } else {
            self.leaves[index] = leaf;
        }
        self.write_path(index, hash);
    }

    fn write_path(&mut self, index: usize, hash: Digest) {
        let write = |level: &mut Vec<Digest>, position: usize, digest: Digest| {
            if position == level.len() {
                level.push(digest);
            } else {
                level[position] = digest;
            }
        };
        write(&mut self.levels[0], index, hash);
        for level in 0..self.depth() {
            let position = index >> level;
            let left = self.node(level, (position & !1) as u64);
            let right = self.node(level, (position | 1) as u64);
            let parent = H::node(&left, &right);
            write(&mut self.levels[level + 1], position >> 1, parent);
        }
    }

    /// Inserts a new key and returns the witness a guest needs.
    pub fn insert_and_generate_witness(
        &mut self,
        key: Digest,
        value: Digest,
    ) -> Result<InsertWitness, IndexedMerkleMapError> {
        if self.order.contains_key(&key) {
            return Err(IndexedMerkleMapError::KeyAlreadyExists);
        }
        if self.length() >= 1u64 << self.depth() {
            return Err(IndexedMerkleMapError::TreeFull);
        }
        let non_membership_proof = self
            .get_non_membership_proof(&key)
            .ok_or(IndexedMerkleMapError::InvalidProof)?;

        let low_leaf = non_membership_proof.low_leaf;
        self.write_leaf(
            non_membership_proof.index as usize,
            Leaf {
                next_key: key,
                ..low_leaf
            },
        );
        let slot = self.leaves.len();
        let slot_proof = self.merkle_proof(slot as u64);
        self.write_leaf(
            slot,
            Leaf {
                key,
                value,
                next_key: low_leaf.next_key,
            },
        );
        self.order.insert(key, slot);
        Ok(InsertWitness {
            non_membership_proof,
            slot_proof,
        })
    }

    /// Changes an existing key's value and returns the witness a guest needs.
    pub fn update_and_generate_witness(
        &mut self,
        key: Digest,
        value: Digest,
    ) -> Result<UpdateWitness, IndexedMerkleMapError> {
        let membership_proof = self
            .get_membership_proof(&key)
            .ok_or(IndexedMerkleMapError::KeyDoesNotExist)?;
        self.write_leaf(
            membership_proof.index as usize,
            Leaf {
                value,
                ..membership_proof.leaf
            },
        );
        Ok(UpdateWitness { membership_proof })
    }

    pub fn insert(&mut self, key: Digest, value: Digest) -> Result<(), IndexedMerkleMapError> {
        self.insert_and_generate_witness(key, value).map(|_| ())
    }

    /// Returns the old value.
    pub fn update(&mut self, key: Digest, value: Digest) -> Result<Digest, IndexedMerkleMapError> {
        let witness = self.update_and_generate_witness(key, value)?;
        Ok(witness.membership_proof.leaf.value)
    }

    /// Inserts or updates, returning the old value if there was one.
    pub fn set(
        &mut self,
        key: Digest,
        value: Digest,
    ) -> Result<Option<Digest>, IndexedMerkleMapError> {
        if self.order.contains_key(&key) {
            self.update(key, value).map(Some)
        } else {
            self.insert(key, value).map(|()| None)
        }
    }
}

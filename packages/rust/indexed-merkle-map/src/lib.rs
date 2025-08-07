extern crate alloc;

use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use crypto_bigint::{Encoding, U256};
use sha2::{Digest, Sha256};
use alloc::collections::BTreeMap;


/// Hash type - 32 bytes SHA256 output
#[derive(Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    pub fn new(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }

    pub fn zero() -> Self {
        Hash([0u8; 32])
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes[..32]);
        Hash(hash)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_u256(&self) -> U256 {
        U256::from_be_bytes(self.0)
    }
}

pub fn sha256_hash(data: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    Hash::from_bytes(&result)
}

/// Hash two hashes together (for merkle tree nodes)
pub fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(left.as_bytes());
    data.extend_from_slice(right.as_bytes());
    sha256_hash(&data)
}

/// Wrapper for U256 to implement serialization traits
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Field([u8; 32]);

impl Field {
    pub fn zero() -> Self {
        Field([0u8; 32])
    }

    pub fn from_u256(val: U256) -> Self {
        Field(val.to_be_bytes())
    }

    pub fn to_u256(&self) -> U256 {
        U256::from_be_bytes(self.0)
    }

    pub fn from_u32(val: u32) -> Self {
        Self::from_u256(U256::from_u32(val))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl BorshSerialize for Field {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(&self.0)?;
        Ok(())
    }
}

impl BorshDeserialize for Field {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; 32];
        reader.read_exact(&mut bytes)?;
        Ok(Field(bytes))
    }
}

/// A leaf in the indexed merkle tree
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Leaf {
    pub key: Field,
    pub value: Field,
    pub next_key: Field,
    pub index: usize,
}

impl Leaf {
    pub fn new(key: Field, value: Field, next_key: Field, index: usize) -> Self {
        Leaf { key, value, next_key, index }
    }

    pub fn empty() -> Self {
        Leaf {
            key: Field::zero(),
            value: Field::zero(),
            next_key: Field::zero(),
            index: 0,
        }
    }

    /// Hash the leaf node for inclusion in the merkle tree
    pub fn hash(&self) -> Hash {
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(self.key.as_bytes());
        data.extend_from_slice(self.value.as_bytes());
        data.extend_from_slice(self.next_key.as_bytes());
        sha256_hash(&data)
    }
}

/// Merkle proof path
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct MerkleProof {
    pub siblings: Vec<Hash>,
    pub path_indices: Vec<bool>, // true = right, false = left
}

/// Proof of membership in the indexed merkle tree
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct MembershipProof {
    pub leaf: Leaf,
    pub merkle_proof: MerkleProof,
}

/// Proof of non-membership in the indexed merkle tree
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct NonMembershipProof {
    pub low_leaf: Leaf,
    pub merkle_proof: MerkleProof,
}

/// The IndexedMerkleMap structure
pub struct IndexedMerkleMap {
    height: usize,
    nodes: Vec<Vec<Hash>>,           // nodes[level][index]
    sorted_leaves: Vec<Leaf>,         // sorted by key
    leaf_indices: BTreeMap<Field, usize>, // key -> leaf index in tree
    root: Hash,
    next_index: usize,
}

impl IndexedMerkleMap {
    /// Create a new IndexedMerkleMap with the given height
    pub fn new(height: usize) -> Self {
        assert!(height > 0 && height <= 32, "Height must be between 1 and 32");

        let mut nodes = Vec::with_capacity(height);
        for _ in 0..height {
            nodes.push(Vec::new());
        }

        // Initialize with first leaf (0, 0, 0)
        let first_leaf = Leaf::empty();
        let first_hash = first_leaf.hash();
        
        nodes[0].push(first_hash);
        
        // Build up the tree to root
        let mut current_hash = first_hash;
        for level in 1..height {
            nodes[level].push(current_hash);
            current_hash = hash_pair(&current_hash, &Hash::zero());
        }

        let mut leaf_indices = BTreeMap::new();
        leaf_indices.insert(Field::zero(), 0);

        IndexedMerkleMap {
            height,
            nodes,
            sorted_leaves: vec![first_leaf],
            leaf_indices,
            root: current_hash,
            next_index: 1,
        }
    }

    /// Get the current merkle root
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Find the leaf with the largest key less than the given key
    fn find_low_leaf(&self, key: &Field) -> &Leaf {
        let mut low = &self.sorted_leaves[0];
        
        for leaf in &self.sorted_leaves {
            if leaf.key < *key && leaf.key > low.key {
                low = leaf;
            }
        }
        
        low
    }

    /// Compute merkle root from leaf and proof
    fn compute_root(leaf_hash: Hash, proof: &MerkleProof) -> Hash {
        let mut current = leaf_hash;
        
        for (sibling, is_right) in proof.siblings.iter().zip(&proof.path_indices) {
            if *is_right {
                current = hash_pair(sibling, &current);
            } else {
                current = hash_pair(&current, sibling);
            }
        }
        
        current
    }

    /// Get merkle proof for a leaf at given index
    fn get_merkle_proof(&self, leaf_index: usize) -> MerkleProof {
        let mut siblings = Vec::new();
        let mut path_indices = Vec::new();
        let mut index = leaf_index;
        
        for level in 0..self.height - 1 {
            let is_right = index % 2 == 1;
            path_indices.push(is_right);
            
            let sibling_index = if is_right { index - 1 } else { index + 1 };
            let sibling = if sibling_index < self.nodes[level].len() {
                self.nodes[level][sibling_index]
            } else {
                Hash::zero()
            };
            siblings.push(sibling);
            
            index /= 2;
        }
        
        MerkleProof { siblings, path_indices }
    }

    /// Update the merkle tree after modifying a leaf
    fn update_tree(&mut self, leaf_index: usize, new_leaf_hash: Hash) {
        self.nodes[0][leaf_index] = new_leaf_hash;
        
        let mut index = leaf_index;
        for level in 0..self.height - 1 {
            let parent_index = index / 2;
            
            let left_child = if parent_index * 2 < self.nodes[level].len() {
                self.nodes[level][parent_index * 2]
            } else {
                Hash::zero()
            };
            
            let right_child = if parent_index * 2 + 1 < self.nodes[level].len() {
                self.nodes[level][parent_index * 2 + 1]
            } else {
                Hash::zero()
            };
            
            let parent_hash = hash_pair(&left_child, &right_child);
            
            if level + 1 == self.height - 1 {
                self.root = parent_hash;
            } else {
                if parent_index >= self.nodes[level + 1].len() {
                    self.nodes[level + 1].resize(parent_index + 1, Hash::zero());
                }
                self.nodes[level + 1][parent_index] = parent_hash;
            }
            
            index = parent_index;
        }
    }

    /// Insert a new key-value pair
    pub fn insert(&mut self, key: Field, value: Field) -> Result<NonMembershipProof, &'static str> {
        // Check if key already exists
        if self.leaf_indices.contains_key(&key) {
            return Err("Key already exists");
        }

        // Find the low leaf
        let low_leaf = self.find_low_leaf(&key).clone();
        
        // Verify that key is between low_leaf.key and low_leaf.next_key
        if key <= low_leaf.key || (low_leaf.next_key != Field::zero() && key >= low_leaf.next_key) {
            return Err("Invalid key position");
        }

        // Get proof for low leaf before modification
        let low_leaf_index = self.leaf_indices[&low_leaf.key];
        let proof_before = self.get_merkle_proof(low_leaf_index);
        let non_membership_proof = NonMembershipProof {
            low_leaf: low_leaf.clone(),
            merkle_proof: proof_before,
        };

        // Create new leaf
        let new_leaf = Leaf::new(key, value, low_leaf.next_key, self.next_index);
        
        // Update low leaf's next_key
        let updated_low_leaf = Leaf::new(low_leaf.key, low_leaf.value, key, low_leaf.index);
        
        // Update tree for low leaf
        self.update_tree(low_leaf_index, updated_low_leaf.hash());
        
        // Update sorted leaves
        for leaf in &mut self.sorted_leaves {
            if leaf.key == low_leaf.key {
                leaf.next_key = key;
                break;
            }
        }
        
        // Insert new leaf in sorted position
        let insert_pos = self.sorted_leaves.iter().position(|l| l.key > key).unwrap_or(self.sorted_leaves.len());
        self.sorted_leaves.insert(insert_pos, new_leaf.clone());
        
        // Add new leaf to the tree
        if self.next_index >= self.nodes[0].len() {
            self.nodes[0].resize(self.next_index + 1, Hash::zero());
        }
        self.nodes[0][self.next_index] = new_leaf.hash();
        self.update_tree(self.next_index, new_leaf.hash());
        
        // Update indices
        self.leaf_indices.insert(key, self.next_index);
        self.next_index += 1;

        Ok(non_membership_proof)
    }

    /// Update the value for an existing key
    pub fn update(&mut self, key: Field, value: Field) -> Result<Field, &'static str> {
        let leaf_index = *self.leaf_indices.get(&key).ok_or("Key does not exist")?;
        
        // Find the leaf in sorted_leaves and update it
        let (old_value, new_hash) = {
            let leaf = self.sorted_leaves.iter_mut()
                .find(|l| l.key == key)
                .ok_or("Leaf not found in sorted list")?;
            
            let old_value = leaf.value;
            leaf.value = value;
            (old_value, leaf.hash())
        };
        
        // Update the tree
        self.update_tree(leaf_index, new_hash);
        
        Ok(old_value)
    }

    /// Get membership proof for a key
    pub fn get_membership_proof(&self, key: &Field) -> Option<MembershipProof> {
        let leaf_index = *self.leaf_indices.get(key)?;
        
        let leaf = self.sorted_leaves.iter()
            .find(|l| l.key == *key)?
            .clone();
        
        let merkle_proof = self.get_merkle_proof(leaf_index);
        
        Some(MembershipProof { leaf, merkle_proof })
    }

    /// Get non-membership proof for a key
    pub fn get_non_membership_proof(&self, key: &Field) -> Option<NonMembershipProof> {
        if self.leaf_indices.contains_key(key) {
            return None; // Key exists, can't prove non-membership
        }
        
        let low_leaf = self.find_low_leaf(key).clone();
        let low_leaf_index = self.leaf_indices[&low_leaf.key];
        let merkle_proof = self.get_merkle_proof(low_leaf_index);
        
        Some(NonMembershipProof { low_leaf, merkle_proof })
    }

    /// Verify a membership proof
    pub fn verify_membership_proof(root: &Hash, proof: &MembershipProof, key: &Field, value: &Field) -> bool {
        if proof.leaf.key != *key || proof.leaf.value != *value {
            return false;
        }
        
        let computed_root = Self::compute_root(proof.leaf.hash(), &proof.merkle_proof);
        computed_root == *root
    }

    /// Verify a non-membership proof
    pub fn verify_non_membership_proof(root: &Hash, proof: &NonMembershipProof, key: &Field) -> bool {
        // Check that key is between low_leaf.key and low_leaf.next_key
        if *key <= proof.low_leaf.key {
            return false;
        }
        
        if proof.low_leaf.next_key != Field::zero() && *key >= proof.low_leaf.next_key {
            return false;
        }
        
        let computed_root = Self::compute_root(proof.low_leaf.hash(), &proof.merkle_proof);
        computed_root == *root
    }

    
    pub fn height(&self) -> usize {
        self.height
    }

    // Test helper methods - only available in test builds
    #[doc(hidden)]
    pub fn sorted_leaves(&self) -> &Vec<Leaf> {
        &self.sorted_leaves
    }

    #[doc(hidden)]
    pub fn next_index(&self) -> usize {
        self.next_index
    }
}
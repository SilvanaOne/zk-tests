//! Merkle module - Wrapper API around indexed-merkle-map crate
//!
//! Provides a clean interface for calculating indexed merkle map roots
//! from key-value pairs without exposing implementation details.

use anyhow::Result;
use crypto_bigint::U256;
use indexed_merkle_map::{Field, IndexedMerkleMap};

/// Calculate the root hash of an indexed merkle map from key-value pairs
///
/// # Arguments
/// * `pairs` - Array of (key, value) integer pairs
///
/// # Returns
/// Hex-encoded root hash of the merkle map
pub fn calculate_root(pairs: &[(i64, i64)]) -> Result<String> {
    // Create new indexed merkle map with height 32 (max capacity)
    let mut map = IndexedMerkleMap::new(32);

    // Insert all key-value pairs
    for &(key, value) in pairs {
        let key_field = Field::from_u256(U256::from_u64(key as u64));
        let value_field = Field::from_u256(U256::from_u64(value as u64));

        map.set(key_field, value_field)?;
    }

    // Get root hash and convert to hex
    let root = map.root();
    Ok(hex::encode(root.as_bytes()))
}

/// Get the root hash of an empty indexed merkle map
///
/// # Returns
/// Hex-encoded root hash of an empty merkle map
pub fn empty_root() -> String {
    let map = IndexedMerkleMap::new(32);
    let root = map.root();
    hex::encode(root.as_bytes())
}

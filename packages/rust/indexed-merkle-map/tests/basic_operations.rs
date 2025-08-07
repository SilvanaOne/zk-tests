use indexed_merkle_map::{Field, Hash, IndexedMerkleMap};

#[test]
fn test_initialization() {
    let map = IndexedMerkleMap::new(10);
    
    // Check initial state
    assert_eq!(map.sorted_leaves().len(), 1);
    assert_eq!(map.sorted_leaves()[0].key, Field::zero());
    assert_eq!(map.sorted_leaves()[0].value, Field::zero());
    assert_eq!(map.sorted_leaves()[0].next_key, Field::zero());
    
    // Root should be non-zero
    assert_ne!(map.root(), Hash::zero());
}

#[test]
fn test_single_insert() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    // Insert should succeed
    let proof = map.insert(key, value);
    assert!(proof.is_ok());
    
    // Verify the key now exists
    let membership_proof = map.get_membership_proof(&key).unwrap();
    assert_eq!(membership_proof.leaf.key, key);
    assert_eq!(membership_proof.leaf.value, value);
}

#[test]
fn test_multiple_inserts_sequential() {
    let mut map = IndexedMerkleMap::new(10);
    
    // Insert keys in sequential order
    for i in 1..=10 {
        let key = Field::from_u32(i * 10);
        let value = Field::from_u32(i * 100);
        map.insert(key, value).expect("Insert should succeed");
    }
    
    // Verify all keys exist
    for i in 1..=10 {
        let key = Field::from_u32(i * 10);
        let value = Field::from_u32(i * 100);
        let proof = map.get_membership_proof(&key).unwrap();
        assert_eq!(proof.leaf.key, key);
        assert_eq!(proof.leaf.value, value);
    }
}

#[test]
fn test_multiple_inserts_random_order() {
    let mut map = IndexedMerkleMap::new(10);
    
    // Insert keys in random order
    let keys = vec![50, 20, 80, 10, 60, 90, 30, 70, 40, 100];
    
    for (i, &k) in keys.iter().enumerate() {
        let key = Field::from_u32(k);
        let value = Field::from_u32((i + 1) as u32 * 100);
        map.insert(key, value).expect("Insert should succeed");
    }
    
    // Verify all keys exist with correct values
    for (i, &k) in keys.iter().enumerate() {
        let key = Field::from_u32(k);
        let value = Field::from_u32((i + 1) as u32 * 100);
        let proof = map.get_membership_proof(&key).unwrap();
        assert_eq!(proof.leaf.key, key);
        assert_eq!(proof.leaf.value, value);
    }
    
    // Verify sorted order is maintained
    let mut prev_key = Field::zero();
    for leaf in map.sorted_leaves() {
        assert!(leaf.key >= prev_key);
        prev_key = leaf.key;
    }
}

#[test]
fn test_update_existing_key() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value1 = Field::from_u32(200);
    let value2 = Field::from_u32(300);
    
    // Insert initial value
    map.insert(key, value1).expect("Insert should succeed");
    
    // Update to new value
    let old_value = map.update(key, value2).expect("Update should succeed");
    assert_eq!(old_value, value1);
    
    // Verify new value
    let proof = map.get_membership_proof(&key).unwrap();
    assert_eq!(proof.leaf.value, value2);
}

#[test]
fn test_update_nonexistent_key() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    // Update should fail for non-existent key
    let result = map.update(key, value);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Key does not exist");
}

#[test]
fn test_insert_duplicate_key() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    // First insert should succeed
    map.insert(key, value).expect("First insert should succeed");
    
    // Second insert with same key should fail
    let result = map.insert(key, value);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Key already exists");
}

#[test]
fn test_get_membership_proof_existing() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    map.insert(key, value).expect("Insert should succeed");
    
    let proof = map.get_membership_proof(&key);
    assert!(proof.is_some());
    
    let proof = proof.unwrap();
    assert_eq!(proof.leaf.key, key);
    assert_eq!(proof.leaf.value, value);
}

#[test]
fn test_get_membership_proof_nonexistent() {
    let map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    
    let proof = map.get_membership_proof(&key);
    assert!(proof.is_none());
}

#[test]
fn test_get_non_membership_proof_nonexistent() {
    let map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    
    let proof = map.get_non_membership_proof(&key);
    assert!(proof.is_some());
    
    let proof = proof.unwrap();
    // For empty tree, low leaf should be the zero leaf
    assert_eq!(proof.low_leaf.key, Field::zero());
    assert!(key > proof.low_leaf.key);
    assert_eq!(proof.low_leaf.next_key, Field::zero());
}

#[test]
fn test_get_non_membership_proof_existing() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    map.insert(key, value).expect("Insert should succeed");
    
    let proof = map.get_non_membership_proof(&key);
    assert!(proof.is_none());
}

#[test]
fn test_root_changes_on_insert() {
    let mut map = IndexedMerkleMap::new(10);
    
    let root1 = map.root();
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    map.insert(key, value).expect("Insert should succeed");
    
    let root2 = map.root();
    
    assert_ne!(root1, root2);
}

#[test]
fn test_root_changes_on_update() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value1 = Field::from_u32(200);
    let value2 = Field::from_u32(300);
    
    map.insert(key, value1).expect("Insert should succeed");
    let root1 = map.root();
    
    map.update(key, value2).expect("Update should succeed");
    let root2 = map.root();
    
    assert_ne!(root1, root2);
}

#[test]
fn test_tree_height_limits() {
    // Test minimum height
    let map = IndexedMerkleMap::new(1);
    assert_eq!(map.height(), 1);
    
    // Test maximum height
    let map = IndexedMerkleMap::new(32);
    assert_eq!(map.height(), 32);
}

#[test]
#[should_panic(expected = "Height must be between 1 and 32")]
fn test_invalid_height_zero() {
    IndexedMerkleMap::new(0);
}

#[test]
#[should_panic(expected = "Height must be between 1 and 32")]
fn test_invalid_height_too_large() {
    IndexedMerkleMap::new(33);
}
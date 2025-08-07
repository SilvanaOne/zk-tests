use indexed_merkle_map::{Field, IndexedMerkleMap};

#[test]
fn test_new_map() {
    let map = IndexedMerkleMap::new(10);
    assert_eq!(map.sorted_leaves().len(), 1);
    assert_eq!(map.sorted_leaves()[0].key, Field::zero());
}

#[test]
fn test_insert() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    // Get root before insertion
    let root_before = map.root();
    
    // Get non-membership proof before insertion
    let proof_before = map.get_non_membership_proof(&key).unwrap();
    assert!(IndexedMerkleMap::verify_non_membership_proof(&root_before, &proof_before, &key));
    
    // Insert the key
    map.insert(key, value).unwrap();
    
    // Now verify membership after insertion
    let proof_after = map.get_membership_proof(&key).unwrap();
    assert!(IndexedMerkleMap::verify_membership_proof(&map.root(), &proof_after, &key, &value));
}

#[test]
fn test_membership_proof() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    
    map.insert(key, value).unwrap();
    
    let proof = map.get_membership_proof(&key).unwrap();
    assert!(IndexedMerkleMap::verify_membership_proof(&map.root(), &proof, &key, &value));
}

#[test]
fn test_update() {
    let mut map = IndexedMerkleMap::new(10);
    
    let key = Field::from_u32(100);
    let value1 = Field::from_u32(200);
    let value2 = Field::from_u32(300);
    
    map.insert(key, value1).unwrap();
    
    let old_value = map.update(key, value2).unwrap();
    assert_eq!(old_value, value1);
    
    let proof = map.get_membership_proof(&key).unwrap();
    assert!(IndexedMerkleMap::verify_membership_proof(&map.root(), &proof, &key, &value2));
}

#[test]
fn test_multiple_inserts() {
    let mut map = IndexedMerkleMap::new(10);
    
    let keys = vec![
        Field::from_u32(100),
        Field::from_u32(50),
        Field::from_u32(150),
        Field::from_u32(75),
    ];
    
    for (i, key) in keys.iter().enumerate() {
        let value = Field::from_u32((i + 1) as u32 * 100);
        map.insert(*key, value).unwrap();
    }
    
    // Verify all keys exist
    for (i, key) in keys.iter().enumerate() {
        let value = Field::from_u32((i + 1) as u32 * 100);
        let proof = map.get_membership_proof(key).unwrap();
        assert!(IndexedMerkleMap::verify_membership_proof(&map.root(), &proof, key, &value));
    }
    
    // Verify non-membership
    let non_existent = Field::from_u32(60);
    let proof = map.get_non_membership_proof(&non_existent).unwrap();
    assert!(IndexedMerkleMap::verify_non_membership_proof(&map.root(), &proof, &non_existent));
}
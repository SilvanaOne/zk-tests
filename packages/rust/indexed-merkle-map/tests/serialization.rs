use indexed_merkle_map::{Field, Hash, IndexedMerkleMap, Leaf, MembershipProof, NonMembershipProof};

#[test]
fn test_field_serialization() {
    let field = Field::from_u32(12345);
    
    // Serialize
    let serialized = borsh::to_vec(&field).unwrap();
    assert_eq!(serialized.len(), 32); // Field is 32 bytes
    
    // Deserialize
    let deserialized: Field = borsh::from_slice(&serialized).unwrap();
    assert_eq!(field, deserialized);
}

#[test]
fn test_hash_serialization() {
    let hash = Hash::new([42u8; 32]);
    
    // Serialize
    let serialized = borsh::to_vec(&hash).unwrap();
    assert_eq!(serialized.len(), 32);
    
    // Deserialize
    let deserialized: Hash = borsh::from_slice(&serialized).unwrap();
    assert_eq!(hash, deserialized);
}

#[test]
fn test_leaf_serialization() {
    let leaf = Leaf::new(
        Field::from_u32(100),
        Field::from_u32(200),
        Field::from_u32(300),
        5,
    );
    
    // Serialize
    let serialized = borsh::to_vec(&leaf).unwrap();
    
    // Deserialize
    let deserialized: Leaf = borsh::from_slice(&serialized).unwrap();
    assert_eq!(leaf, deserialized);
    assert_eq!(deserialized.key, Field::from_u32(100));
    assert_eq!(deserialized.value, Field::from_u32(200));
    assert_eq!(deserialized.next_key, Field::from_u32(300));
    assert_eq!(deserialized.index, 5);
}

#[test]
fn test_membership_proof_serialization() {
    let mut map = IndexedMerkleMap::new(10);
    
    // Insert some data
    let key = Field::from_u32(100);
    let value = Field::from_u32(200);
    map.insert(key, value).unwrap();
    
    // Get membership proof
    let proof = map.get_membership_proof(&key).unwrap();
    
    // Serialize
    let serialized = borsh::to_vec(&proof).unwrap();
    
    // Deserialize
    let deserialized: MembershipProof = borsh::from_slice(&serialized).unwrap();
    
    // Verify fields match
    assert_eq!(proof.leaf, deserialized.leaf);
    assert_eq!(proof.merkle_proof.siblings.len(), deserialized.merkle_proof.siblings.len());
    assert_eq!(proof.merkle_proof.path_indices, deserialized.merkle_proof.path_indices);
    
    // Verify deserialized proof still works
    let root = map.root();
    assert!(IndexedMerkleMap::verify_membership_proof(&root, &deserialized, &key, &value));
}

#[test]
fn test_non_membership_proof_serialization() {
    let mut map = IndexedMerkleMap::new(10);
    
    // Insert some data with gap
    map.insert(Field::from_u32(10), Field::from_u32(100)).unwrap();
    map.insert(Field::from_u32(30), Field::from_u32(300)).unwrap();
    
    // Get non-membership proof for key in gap
    let non_existent = Field::from_u32(20);
    let proof = map.get_non_membership_proof(&non_existent).unwrap();
    
    // Serialize
    let serialized = borsh::to_vec(&proof).unwrap();
    
    // Deserialize
    let deserialized: NonMembershipProof = borsh::from_slice(&serialized).unwrap();
    
    // Verify fields match
    assert_eq!(proof.low_leaf, deserialized.low_leaf);
    assert_eq!(proof.merkle_proof.siblings.len(), deserialized.merkle_proof.siblings.len());
    
    // Verify deserialized proof still works
    let root = map.root();
    assert!(IndexedMerkleMap::verify_non_membership_proof(&root, &deserialized, &non_existent));
}

#[test]
fn test_large_proof_serialization() {
    let mut map = IndexedMerkleMap::new(20); // Large tree
    
    // Insert many keys
    for i in 1..=100 {
        let key = Field::from_u32(i * 10);
        let value = Field::from_u32(i * 100);
        map.insert(key, value).unwrap();
    }
    
    // Get proof for middle key
    let key = Field::from_u32(500);
    let value = Field::from_u32(5000);
    let proof = map.get_membership_proof(&key).unwrap();
    
    // Serialize
    let serialized = borsh::to_vec(&proof).unwrap();
    
    // Check size is reasonable
    assert!(serialized.len() < 10000); // Should be much smaller
    
    // Deserialize and verify
    let deserialized: MembershipProof = borsh::from_slice(&serialized).unwrap();
    let root = map.root();
    assert!(IndexedMerkleMap::verify_membership_proof(&root, &deserialized, &key, &value));
}

#[test]
fn test_zero_field_serialization() {
    let zero = Field::zero();
    
    // Serialize
    let serialized = borsh::to_vec(&zero).unwrap();
    assert_eq!(serialized, vec![0u8; 32]);
    
    // Deserialize
    let deserialized: Field = borsh::from_slice(&serialized).unwrap();
    assert_eq!(zero, deserialized);
}

#[test]
fn test_max_field_serialization() {
    use crypto_bigint::U256;
    
    let max = Field::from_u256(U256::MAX);
    
    // Serialize
    let serialized = borsh::to_vec(&max).unwrap();
    assert_eq!(serialized.len(), 32);
    
    // Deserialize
    let deserialized: Field = borsh::from_slice(&serialized).unwrap();
    assert_eq!(max, deserialized);
}

#[test]
fn test_proof_size_scaling() {
    println!("\nProof size scaling test:");
    
    for height in [5, 10, 15, 20] {
        let mut map = IndexedMerkleMap::new(height);
        
        // Insert a key
        let key = Field::from_u32(100);
        let value = Field::from_u32(200);
        map.insert(key, value).unwrap();
        
        // Get proof
        let proof = map.get_membership_proof(&key).unwrap();
        
        // Serialize
        let serialized = borsh::to_vec(&proof).unwrap();
        
        println!(
            "Height {}: Proof size = {} bytes ({} siblings)",
            height,
            serialized.len(),
            proof.merkle_proof.siblings.len()
        );
        
        // Expected size: leaf (32*3 + 8) + siblings (32 * (height-1)) + path_indices
        let expected_min = 32 * 3 + 8 + 32 * (height - 1);
        assert!(serialized.len() >= expected_min);
    }
}

#[test]
fn test_corrupted_data_deserialization() {
    let field = Field::from_u32(12345);
    let mut serialized = borsh::to_vec(&field).unwrap();
    
    // Corrupt the data
    serialized.truncate(30); // Make it too short
    
    // Deserialization should fail
    let result: Result<Field, _> = borsh::from_slice(&serialized);
    assert!(result.is_err());
}

#[test]
fn test_empty_proof_serialization() {
    // Create a minimal proof
    let leaf = Leaf::empty();
    let merkle_proof = indexed_merkle_map::MerkleProof {
        siblings: vec![],
        path_indices: vec![],
    };
    let proof = MembershipProof {
        leaf,
        merkle_proof,
    };
    
    // Serialize
    let serialized = borsh::to_vec(&proof).unwrap();
    
    // Deserialize
    let deserialized: MembershipProof = borsh::from_slice(&serialized).unwrap();
    
    assert_eq!(proof.leaf, deserialized.leaf);
    assert!(deserialized.merkle_proof.siblings.is_empty());
    assert!(deserialized.merkle_proof.path_indices.is_empty());
}

#[test]
fn test_batch_serialization() {
    let mut map = IndexedMerkleMap::new(10);
    
    // Insert multiple keys
    let keys = vec![10, 20, 30, 40, 50];
    for k in &keys {
        let key = Field::from_u32(*k);
        let value = Field::from_u32(*k * 10);
        map.insert(key, value).unwrap();
    }
    
    // Collect all proofs
    let proofs: Vec<MembershipProof> = keys
        .iter()
        .map(|k| {
            let key = Field::from_u32(*k);
            map.get_membership_proof(&key).unwrap()
        })
        .collect();
    
    // Serialize batch
    let serialized = borsh::to_vec(&proofs).unwrap();
    
    // Deserialize batch
    let deserialized: Vec<MembershipProof> = borsh::from_slice(&serialized).unwrap();
    
    assert_eq!(proofs.len(), deserialized.len());
    
    // Verify all deserialized proofs
    let root = map.root();
    for (k, proof) in keys.iter().zip(&deserialized) {
        let key = Field::from_u32(*k);
        let value = Field::from_u32(*k * 10);
        assert!(IndexedMerkleMap::verify_membership_proof(&root, proof, &key, &value));
    }
}
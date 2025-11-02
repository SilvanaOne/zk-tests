//! A program that verifies multiple price proofs and stores verified prices in IndexedMerkleMap
//! Takes multiple PriceProofData entries, verifies all components, inserts into map, outputs roots

#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::{sol, SolType, private::U256};
use borsh::BorshDeserialize;
use price_lib::{verify_all, PriceProofData, price_to_bytes, timestamp_to_bytes};
use indexed_merkle_map::{Hash, InsertWitness, ProvableIndexedMerkleMap, Field};

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        uint256 old_root;
        uint256 new_root;
    }
}

pub fn main() {
    // 1. Read old root
    let old_root_bytes: [u8; 32] = sp1_zkvm::io::read();
    let initial_root = Hash::from_bytes(old_root_bytes);
    let mut current_root = initial_root;

    // 2. Read number of price proofs
    let num_prices: u32 = sp1_zkvm::io::read();

    // 3. Process each price proof
    for i in 0..num_prices {
        // Read PriceProofData
        let proof_bytes: Vec<u8> = sp1_zkvm::io::read();
        let proof_data: PriceProofData = PriceProofData::deserialize(&mut &proof_bytes[..])
            .expect("Failed to deserialize PriceProofData");

        // Verify all proof components
        let verification = verify_all(
            &proof_data.certificates,
            &proof_data.checkpoint,
            &proof_data.tsa_timestamp,
            proof_data.price.timestamp_fetched,
        ).expect("Verification failed");

        // Assert all verifications passed
        assert!(
            verification.all_verified,
            "Verification failed at price {} : not all checks passed", i
        );

        // Convert verified price data to Field values
        let expected_key = Field::from_bytes(timestamp_to_bytes(proof_data.price.timestamp_fetched));
        let expected_value = Field::from_bytes(
            price_to_bytes(&proof_data.price.price).expect("Failed to convert price to bytes")
        );

        // Read the insert witness
        let witness_bytes: Vec<u8> = sp1_zkvm::io::read();
        let witness: InsertWitness = InsertWitness::deserialize(&mut &witness_bytes[..])
            .expect("Failed to deserialize witness");

        // Verify witness matches verified price data
        assert_eq!(witness.old_root, current_root, "Root mismatch at price {}", i);
        assert_eq!(witness.key, expected_key, "Key mismatch at price {}: witness key doesn't match verified timestamp", i);
        assert_eq!(witness.value, expected_value, "Value mismatch at price {}: witness value doesn't match verified price", i);

        // Verify and apply the insert
        ProvableIndexedMerkleMap::insert(&witness).expect("Insert verification failed");

        // Update current root for next iteration
        current_root = witness.new_root;
    }

    // 4. Encode and commit public values (initial_root, final current_root)
    let initial_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
    let old_root_u256 = U256::from_be_bytes(initial_root_bytes);
    let current_root_bytes: [u8; 32] = current_root.as_bytes().try_into().unwrap();
    let new_root_u256 = U256::from_be_bytes(current_root_bytes);

    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct {
        old_root: old_root_u256,
        new_root: new_root_u256,
    });

    sp1_zkvm::io::commit_slice(&bytes);
}

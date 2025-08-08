//! A program that processes account operations using IndexedMerkleMap
//! Takes an old root and array of account actions, verifies and produces new root

#![no_main]
sp1_zkvm::entrypoint!(main);

use indexed_merkle_map::{Hash, ProvableIndexedMerkleMap, InsertWitness, UpdateWitness};
use alloy_sol_types::{SolType, private::U256};
use add_lib::PublicValuesStruct;
use borsh::BorshDeserialize;

#[derive(BorshDeserialize)]
enum AccountAction {
    Insert(InsertWitness),
    Update(UpdateWitness),
}

pub fn main() {
    // Read the old root
    let old_root_bytes: [u8; 32] = sp1_zkvm::io::read();
    let initial_root = Hash::from_bytes(&old_root_bytes);
    let mut current_root = initial_root;
    
    // Read the number of actions
    let num_actions: u32 = sp1_zkvm::io::read();
    
    // Process each action
    for i in 0..num_actions {
        // Read the serialized action
        let action_bytes: Vec<u8> = sp1_zkvm::io::read();
        let action: AccountAction = AccountAction::deserialize(&mut &action_bytes[..])
            .expect("Failed to deserialize action");
        
        // Verify and apply the action
        match action {
            AccountAction::Insert(witness) => {
                // Verify the old root matches
                assert_eq!(witness.old_root, current_root, "Root mismatch at action {}", i);
                
                // Verify the insertion
                ProvableIndexedMerkleMap::insert(&witness)
                    .expect("Failed to verify insert");
                
                // Update current root
                current_root = witness.new_root;
            }
            AccountAction::Update(witness) => {
                // Verify the old root matches
                assert_eq!(witness.old_root, current_root, "Root mismatch at action {}", i);
                
                // Verify the update
                ProvableIndexedMerkleMap::update(&witness)
                    .expect("Failed to verify update");
                
                // Update current root
                current_root = witness.new_root;
            }
        }
    }
    
    // Convert roots to U256 for PublicValuesStruct
    let old_root_bytes: [u8; 32] = initial_root.as_bytes().try_into().unwrap();
    let old_root_u256 = U256::from_be_bytes(old_root_bytes);
    let new_root_bytes: [u8; 32] = current_root.as_bytes().try_into().unwrap();
    let new_root_u256 = U256::from_be_bytes(new_root_bytes);
    
    // Encode the public values
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct { 
        old_root: old_root_u256, 
        new_root: new_root_u256 
    });
    
    // Commit to the public values
    sp1_zkvm::io::commit_slice(&bytes);
}
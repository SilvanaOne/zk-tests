//! AddMapElement command handler - Add elements to indexed merkle map in Daml contract

use anyhow::{Context, Result};
use crypto_bigint::U256;
use indexed_merkle_map::{Field, IndexedMerkleMap};
use serde_json::json;
use tracing::info;

use crate::{contract, url};

/// Handle the addmapelement command
///
/// # Arguments
/// * `key` - Key to insert (i64)
/// * `value` - Value to insert (i64)
///
/// # Returns
/// Result indicating success or failure
pub async fn handle_addmapelement(key: i64, value: i64) -> Result<()> {
    info!("Adding map element: key={}, value={}", key, value);

    // Load environment variables
    let api_url = std::env::var("APP_USER_API_URL")
        .context("APP_USER_API_URL not set in environment")?;
    let jwt = std::env::var("APP_USER_JWT")
        .context("APP_USER_JWT not set in environment")?;
    let party_app_user = std::env::var("PARTY_APP_USER")
        .context("PARTY_APP_USER not set in environment")?;
    let hash_package_id = std::env::var("HASH_PACKAGE_ID")
        .context("HASH_PACKAGE_ID not set in environment")?;

    let template_id = format!("{}:Hash:Hash", hash_package_id);

    // Create HTTP client with localhost resolution
    let client = url::create_client_with_localhost_resolution()?;

    // Create a new IndexedMerkleMap (height 32 for max capacity)
    let mut map = IndexedMerkleMap::new(32);

    // Convert key/value to Field
    let key_field = Field::from_u256(U256::from_u64(key as u64));
    let value_field = Field::from_u256(U256::from_u64(value as u64));

    info!("Generating witness for insertion...");

    // Generate witness
    let witness = map.insert_and_generate_witness(key_field, value_field, true)?
        .ok_or_else(|| anyhow::anyhow!("Failed to generate witness"))?;

    info!("Witness generated successfully");
    info!("  Old root: {}", hex::encode(witness.old_root.as_bytes()));
    info!("  New root: {}", hex::encode(witness.new_root.as_bytes()));

    // Convert witness to Daml-compatible JSON format
    let witness_json = convert_witness_to_daml_json(&witness)?;

    info!("Creating Hash contract...");

    // Create Hash contract
    let contract_id = contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
    ).await?;

    info!("Exercising AddMapElement choice...");

    // Exercise AddMapElement choice
    let update_id = contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &contract_id,
        "AddMapElement",
        witness_json,
    ).await?;

    // Get and display the update
    let update = contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &update_id,
    ).await?;

    println!("\n=== AddMapElement Update ===");
    println!("{}", serde_json::to_string_pretty(&update)?);

    // Extract and display the new root from contract state
    if let Some(events) = update
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(root) = created.pointer("/createArgument/root").and_then(|v| v.as_str()) {
                    println!("\n=== Result ===");
                    println!("Key: {}", key);
                    println!("Value: {}", value);
                    println!("New root in contract: {}", root);
                    println!("Expected new root:     {}", hex::encode(witness.new_root.as_bytes()));

                    if root == hex::encode(witness.new_root.as_bytes()) {
                        println!("✅ Roots match!");
                    } else {
                        println!("❌ Root mismatch!");
                    }
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Convert IndexedMerkleMap InsertWitness to Daml-compatible JSON format
fn convert_witness_to_daml_json(witness: &indexed_merkle_map::InsertWitness) -> Result<serde_json::Value> {
    // Helper to convert Field to 64-char hex string
    let field_to_hex = |f: &Field| {
        let bytes = f.as_bytes();
        hex::encode(bytes)
    };

    // Helper to convert Hash to 64-char hex string
    let hash_to_hex = |h: &indexed_merkle_map::Hash| {
        hex::encode(h.as_bytes())
    };

    // Helper to convert Leaf to JSON
    let leaf_to_json = |leaf: &indexed_merkle_map::Leaf| {
        json!({
            "key": field_to_hex(&leaf.key),
            "value": field_to_hex(&leaf.value),
            "nextKey": field_to_hex(&leaf.next_key),
            "index": leaf.index
        })
    };

    // Helper to convert MerkleProof to JSON
    let proof_to_json = |proof: &indexed_merkle_map::MerkleProof| {
        json!({
            "siblings": proof.siblings.iter().map(hash_to_hex).collect::<Vec<_>>(),
            "pathIndices": proof.path_indices.clone()
        })
    };

    // Build the complete witness JSON
    let witness_json = json!({
        "witness": {
            "oldRoot": hash_to_hex(&witness.old_root),
            "newRoot": hash_to_hex(&witness.new_root),
            "key": field_to_hex(&witness.key),
            "value": field_to_hex(&witness.value),
            "newLeafIndex": witness.new_leaf_index,
            "treeLength": witness.tree_length,
            "lowLeaf": leaf_to_json(&witness.non_membership_proof.low_leaf),
            "lowLeafProof": proof_to_json(&witness.low_leaf_proof_before),
            "updatedLowLeaf": leaf_to_json(&witness.updated_low_leaf),
            "newLeaf": leaf_to_json(&witness.new_leaf),
            "newLeafProofAfter": proof_to_json(&witness.new_leaf_proof_after)
        }
    });

    Ok(witness_json)
}

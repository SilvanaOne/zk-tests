use anyhow::Result;
use indexed_merkle_map::{Field, IndexedMerkleMap};
use serde_json::json;
use tracing::info;

pub async fn handle_updatemapelement(key: i64, value1: i64, value2: i64) -> Result<()> {
    info!("Updating map element: key={}, value1={}, value2={}", key, value1, value2);

    // Get environment variables
    let party_app_user = std::env::var("PARTY_APP_USER")
        .map_err(|_| anyhow::anyhow!("PARTY_APP_USER not set in environment"))?;

    let api_url = std::env::var("APP_USER_API_URL")
        .map_err(|_| anyhow::anyhow!("APP_USER_API_URL not set in environment"))?;

    let jwt = std::env::var("APP_USER_JWT")
        .map_err(|_| anyhow::anyhow!("APP_USER_JWT not set in environment"))?;

    let package_id = std::env::var("HASH_PACKAGE_ID")
        .map_err(|_| anyhow::anyhow!("HASH_PACKAGE_ID not set in environment"))?;

    let template_id = format!("{}:Hash:Hash", package_id);

    // Create HTTP client
    let client = crate::url::create_client_with_localhost_resolution()?;

    // Initialize the indexed merkle map (height 32)
    let mut map = IndexedMerkleMap::new(32);
    info!("Initialized IndexedMerkleMap with height 32");

    let key_field = Field::from_u256(crypto_bigint::U256::from_u64(key as u64));
    let value1_field = Field::from_u256(crypto_bigint::U256::from_u64(value1 as u64));
    let value2_field = Field::from_u256(crypto_bigint::U256::from_u64(value2 as u64));

    // ============ PHASE 1: INSERT ============
    info!("\n=== PHASE 1: INSERT key={} with value={} ===", key, value1);

    // Generate insert witness
    let insert_witness = map.insert_and_generate_witness(key_field, value1_field, true)?
        .ok_or_else(|| anyhow::anyhow!("Failed to generate insert witness"))?;
    let rust_root_after_insert = map.root();
    info!("Rust calculated root after insert: {}", hex::encode(rust_root_after_insert.to_bytes()));

    // Create Hash contract
    let (hash_contract_id, _create_update) = crate::contract::create_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
    ).await?;

    // Convert insert witness to Daml JSON
    let insert_witness_json = convert_insert_witness_to_daml_json(&insert_witness)?;

    // Exercise AddMapElement choice
    let add_update_id = crate::contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &hash_contract_id,
        "AddMapElement",
        insert_witness_json,
    ).await?;

    // Get the insert result
    let insert_result_json = crate::contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &add_update_id,
    ).await?;

    println!("\n=== INSERT Transaction Update ===");
    println!("{}", serde_json::to_string_pretty(&insert_result_json)?);

    // Extract contract ID and root from insert result
    let (new_contract_id, daml_root_after_insert) = extract_contract_info(&insert_result_json, &package_id)?;

    println!("\n=== INSERT Result ===");
    println!("Key: {}, Value: {}", key, value1);
    println!("Rust calculated root: {}", hex::encode(rust_root_after_insert.to_bytes()));
    println!("Daml calculated root: {}", daml_root_after_insert);

    if hex::encode(rust_root_after_insert.to_bytes()) == daml_root_after_insert {
        println!("✅ INSERT roots match!");
    } else {
        println!("❌ INSERT roots DO NOT match!");
    }

    // ============ PHASE 2: UPDATE ============
    info!("\n=== PHASE 2: UPDATE key={} from value={} to value={} ===", key, value1, value2);

    // Generate update witness
    let update_witness = map.update_and_generate_witness(key_field, value2_field, true)?
        .ok_or_else(|| anyhow::anyhow!("Failed to generate update witness"))?;
    let rust_root_after_update = map.root();
    info!("Rust calculated root after update: {}", hex::encode(rust_root_after_update.to_bytes()));

    // Convert update witness to Daml JSON
    let update_witness_json = convert_update_witness_to_daml_json(&update_witness)?;

    // Exercise UpdateMapElement choice
    let update_update_id = crate::contract::exercise_choice(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &new_contract_id,
        "UpdateMapElement",
        update_witness_json,
    ).await?;

    // Get the update result
    let update_result_json = crate::contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &update_update_id,
    ).await?;

    println!("\n=== UPDATE Transaction Update ===");
    println!("{}", serde_json::to_string_pretty(&update_result_json)?);

    // Extract root from update result
    let (_, daml_root_after_update) = extract_contract_info(&update_result_json, &package_id)?;

    println!("\n=== UPDATE Result ===");
    println!("Key: {}, Old Value: {}, New Value: {}", key, value1, value2);
    println!("Rust calculated root: {}", hex::encode(rust_root_after_update.to_bytes()));
    println!("Daml calculated root: {}", daml_root_after_update);

    if hex::encode(rust_root_after_update.to_bytes()) == daml_root_after_update {
        println!("✅ UPDATE roots match!");
    } else {
        println!("❌ UPDATE roots DO NOT match!");
    }

    Ok(())
}

fn convert_insert_witness_to_daml_json(witness: &indexed_merkle_map::InsertWitness) -> Result<serde_json::Value> {
    let witness_json = json!({
        "witness": {
            "oldRoot": hash_to_hex(&witness.old_root),
            "newRoot": hash_to_hex(&witness.new_root),
            "key": field_to_hex(&witness.key),
            "value": field_to_hex(&witness.value),
            "treeLength": witness.tree_length,
            "newLeafIndex": witness.new_leaf_index,
            "lowLeaf": {
                "key": field_to_hex(&witness.non_membership_proof.low_leaf.key),
                "value": field_to_hex(&witness.non_membership_proof.low_leaf.value),
                "nextKey": field_to_hex(&witness.non_membership_proof.low_leaf.next_key),
                "index": witness.non_membership_proof.low_leaf.index
            },
            "lowLeafProof": {
                "siblings": witness.low_leaf_proof_before.siblings.iter().map(hash_to_hex).collect::<Vec<_>>(),
                "pathIndices": witness.low_leaf_proof_before.path_indices.clone()
            },
            "updatedLowLeaf": {
                "key": field_to_hex(&witness.updated_low_leaf.key),
                "value": field_to_hex(&witness.updated_low_leaf.value),
                "nextKey": field_to_hex(&witness.updated_low_leaf.next_key),
                "index": witness.updated_low_leaf.index
            },
            "newLeaf": {
                "key": field_to_hex(&witness.new_leaf.key),
                "value": field_to_hex(&witness.new_leaf.value),
                "nextKey": field_to_hex(&witness.new_leaf.next_key),
                "index": witness.new_leaf.index
            },
            "newLeafProofAfter": {
                "siblings": witness.new_leaf_proof_after.siblings.iter().map(hash_to_hex).collect::<Vec<_>>(),
                "pathIndices": witness.new_leaf_proof_after.path_indices.clone()
            }
        }
    });
    Ok(witness_json)
}

fn convert_update_witness_to_daml_json(witness: &indexed_merkle_map::UpdateWitness) -> Result<serde_json::Value> {
    let witness_json = json!({
        "witness": {
            "oldRoot": hash_to_hex(&witness.old_root),
            "newRoot": hash_to_hex(&witness.new_root),
            "key": field_to_hex(&witness.key),
            "oldValue": field_to_hex(&witness.old_value),
            "newValue": field_to_hex(&witness.new_value),
            "treeLength": witness.tree_length,
            "oldLeaf": {
                "key": field_to_hex(&witness.membership_proof.leaf.key),
                "value": field_to_hex(&witness.membership_proof.leaf.value),
                "nextKey": field_to_hex(&witness.membership_proof.leaf.next_key),
                "index": witness.membership_proof.leaf.index
            },
            "oldLeafProof": {
                "siblings": witness.membership_proof.merkle_proof.siblings.iter().map(hash_to_hex).collect::<Vec<_>>(),
                "pathIndices": witness.membership_proof.merkle_proof.path_indices.clone()
            },
            "updatedLeaf": {
                "key": field_to_hex(&witness.updated_leaf.key),
                "value": field_to_hex(&witness.updated_leaf.value),
                "nextKey": field_to_hex(&witness.updated_leaf.next_key),
                "index": witness.updated_leaf.index
            }
        }
    });
    Ok(witness_json)
}

fn hash_to_hex(hash: &indexed_merkle_map::Hash) -> String {
    hex::encode(hash.to_bytes())
}

fn field_to_hex(field: &indexed_merkle_map::Field) -> String {
    hex::encode(field.to_bytes())
}

fn extract_contract_info(result_json: &serde_json::Value, package_id: &str) -> Result<(String, String)> {
    if let Some(events) = result_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(&format!("{}:Hash:Hash", package_id)) {
                        let contract_id = created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("Contract ID not found"))?
                            .to_string();

                        let root = created
                            .pointer("/createArgument/root")
                            .and_then(|v| v.as_str())
                            .unwrap_or("null")
                            .to_string();

                        return Ok((contract_id, root));
                    }
                }
            }
        }
    }
    Err(anyhow::anyhow!("Contract info not found in result"))
}

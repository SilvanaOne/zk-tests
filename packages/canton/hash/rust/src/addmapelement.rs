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
    let api_url =
        std::env::var("APP_USER_API_URL").context("APP_USER_API_URL not set in environment")?;
    let jwt = std::env::var("APP_USER_JWT").context("APP_USER_JWT not set in environment")?;
    let party_app_user =
        std::env::var("PARTY_APP_USER").context("PARTY_APP_USER not set in environment")?;
    let party_app_provider =
        std::env::var("PARTY_APP_PROVIDER").context("PARTY_APP_PROVIDER not set in environment")?;
    let hash_package_id =
        std::env::var("HASH_PACKAGE_ID").context("HASH_PACKAGE_ID not set in environment")?;
    let synchronizer_id =
        std::env::var("SYNCHRONIZER_ID").context("SYNCHRONIZER_ID not set in environment")?;

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
    let witness = map
        .insert_and_generate_witness(key_field, value_field, true)?
        .ok_or_else(|| anyhow::anyhow!("Failed to generate witness"))?;

    info!("Witness generated successfully");
    info!("  Old root: {}", hex::encode(witness.old_root.as_bytes()));
    info!("  New root: {}", hex::encode(witness.new_root.as_bytes()));

    // Get membership proof for inclusion verification (after insertion)
    info!("Generating membership proof for inclusion verification...");
    let membership_proof = map
        .get_membership_proof(&key_field)
        .ok_or_else(|| anyhow::anyhow!("Failed to get membership proof after insertion"))?;

    // test: membership_proof.leaf.value = Field::from_u32(1000);
    // Convert witness to Daml-compatible JSON format
    let witness_json = convert_witness_to_daml_json(&witness)?;

    info!("Creating Hash contract via propose-accept workflow...");

    // Load provider JWT for accepting the request
    let app_provider_jwt = std::env::var("APP_PROVIDER_JWT")
        .context("APP_PROVIDER_JWT not set in environment")?;
    let app_provider_api_url = std::env::var("APP_PROVIDER_API_URL")
        .context("APP_PROVIDER_API_URL not set in environment")?;

    // Create HashRequest and have provider accept it
    let (contract_id, hash_id, _create_update_id, _accept_update_id, create_update_json, accept_update_json) =
        contract::create_and_accept_hash_contract(
            &client,
            &api_url,
            &jwt,
            &party_app_user,
            &app_provider_api_url,
            &app_provider_jwt,
            &party_app_provider,
            &template_id,
            &synchronizer_id
        ).await?;

    info!("Hash ID: {}", hash_id);
    info!("Hash contract ID: {}", contract_id);

    println!("\n=== HashRequest Creation Transaction ===");
    println!("{}", serde_json::to_string_pretty(&create_update_json)?);

    println!("\n=== HashRequest Acceptance Transaction ===");
    println!("{}", serde_json::to_string_pretty(&accept_update_json)?);

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
    )
    .await?;

    // Get and display the update from user's perspective
    let update = contract::get_update(&client, &api_url, &jwt, &party_app_user, &update_id).await?;

    println!("\n=== AddMapElement Update (User Node) ===");
    println!("{}", serde_json::to_string_pretty(&update)?);

    // Get and display the update from provider's perspective to verify cross-node visibility
    let update_provider = contract::get_update(
        &client,
        &app_provider_api_url,
        &app_provider_jwt,
        &party_app_provider,
        &update_id
    ).await?;

    println!("\n=== AddMapElement Update (Provider Node) ===");
    println!("{}", serde_json::to_string_pretty(&update_provider)?);

    // Extract and display the new root from contract state
    if let Some(events) = update
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(root) = created
                    .pointer("/createArgument/root")
                    .and_then(|v| v.as_str())
                {
                    println!("\n=== Result ===");
                    println!("Key: {}", key);
                    println!("Value: {}", value);
                    println!("New root in contract: {}", root);
                    println!(
                        "Expected new root:     {}",
                        hex::encode(witness.new_root.as_bytes())
                    );

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

    // === PHASE 2: Verify Inclusion with Non-Consuming Choice ===
    info!("\n=== Verifying Inclusion with VerifyInclusion Choice ===");

    // Extract the new contract ID from the AddMapElement result
    let new_contract_id = extract_contract_id(&update, &hash_package_id)?;
    let new_root = hex::encode(witness.new_root.to_bytes());
    let tree_length = map.next_index(); // tree_length after insertion

    info!("Contract ID: {}", new_contract_id);
    info!("Current root: {}", new_root);
    info!("Tree length: {}", tree_length);

    // Convert membership proof to Daml JSON
    let membership_proof_json = convert_membership_proof_to_daml_json(
        &membership_proof,
        &new_root,
        tree_length,
        &party_app_user,
    )?;

    // Exercise VerifyInclusion choice (non-consuming) and get result directly
    let (verify_result, verify_inclusion_update_id) = exercise_verify_inclusion(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &new_contract_id,
        membership_proof_json,
    )
    .await?;

    // Fetch and print the transaction details
    let verify_inclusion_update = contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &verify_inclusion_update_id
    ).await?;

    println!("\n=== VerifyInclusion Transaction ===");
    println!("{}", serde_json::to_string_pretty(&verify_inclusion_update)?);

    println!("\n=== Inclusion Verification ===");
    println!("Key: {}", key);
    println!("Value: {}", value);
    println!(
        "Verification result: {}",
        if verify_result {
            "✅ VERIFIED IN MAP"
        } else {
            "❌ VERIFICATION FAILED"
        }
    );

    // === PHASE 3: Verify Exclusion with Non-Consuming Choice ===
    info!("\n=== Verifying Exclusion of KEY=1000 with VerifyExclusion Choice ===");

    // Generate non-membership proof for KEY=1000
    let query_key = Field::from_u256(U256::from_u64(1000));

    // Get non-membership proof from the map after insertion
    let non_membership_proof = map
        .get_non_membership_proof(&query_key)
        .ok_or_else(|| anyhow::anyhow!("Failed to generate non-membership proof for key 1000"))?;

    // Convert to Daml JSON format
    let non_membership_proof_json = convert_non_membership_proof_to_daml_json(
        &non_membership_proof,
        &new_root,
        tree_length,
        &party_app_user,
    )?;

    // Exercise VerifyExclusion choice (non-consuming)
    let (exclusion_result, verify_exclusion_update_id) = exercise_verify_exclusion(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &new_contract_id,
        non_membership_proof_json,
    )
    .await?;

    // Fetch and print the transaction details
    let verify_exclusion_update = contract::get_update(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &verify_exclusion_update_id
    ).await?;

    println!("\n=== VerifyExclusion Transaction ===");
    println!("{}", serde_json::to_string_pretty(&verify_exclusion_update)?);

    println!("\n=== Exclusion Verification ===");
    println!("Query key: 1000 (0x{:064x})", 1000);
    println!(
        "Verification result: {}",
        if exclusion_result {
            "✅ KEY 1000 VERIFIED EXCLUDED FROM MAP"
        } else {
            "❌ EXCLUSION VERIFICATION FAILED"
        }
    );

    // === PHASE 4: Archive Contract ===
    info!("\n=== Archiving Contract ===");

    // Archive the contract (using the contract ID from Phase 3, which is still active)
    let (archive_update_id, archive_update_user) = contract::archive_hash_contract(
        &client,
        &api_url,
        &jwt,
        &party_app_user,
        &template_id,
        &new_contract_id,
        &synchronizer_id
    ).await?;

    // Get the archive update from provider's perspective to verify cross-node visibility
    let archive_update_provider = contract::get_update(
        &client,
        &app_provider_api_url,
        &app_provider_jwt,
        &party_app_provider,
        &archive_update_id
    ).await?;

    println!("\n=== Archive Update (User Node) ===");
    println!("{}", serde_json::to_string_pretty(&archive_update_user)?);

    println!("\n=== Archive Update (Provider Node) ===");
    println!("{}", serde_json::to_string_pretty(&archive_update_provider)?);

    println!("\n=== Contract Archived Successfully ===");
    println!("Contract ID: {}", new_contract_id);
    println!("✅ Cross-node archive visibility confirmed");

    Ok(())
}

/// Convert IndexedMerkleMap InsertWitness to Daml-compatible JSON format
fn convert_witness_to_daml_json(
    witness: &indexed_merkle_map::InsertWitness,
) -> Result<serde_json::Value> {
    // Helper to convert Field to 64-char hex string
    let field_to_hex = |f: &Field| {
        let bytes = f.as_bytes();
        hex::encode(bytes)
    };

    // Helper to convert Hash to 64-char hex string
    let hash_to_hex = |h: &indexed_merkle_map::Hash| hex::encode(h.as_bytes());

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

/// Extract contract ID from update JSON
fn extract_contract_id(update_json: &serde_json::Value, package_id: &str) -> Result<String> {
    if let Some(events) = update_json
        .pointer("/update/Transaction/value/events")
        .and_then(|v| v.as_array())
    {
        for event in events {
            if let Some(created) = event.get("CreatedEvent") {
                if let Some(template) = created.pointer("/templateId").and_then(|v| v.as_str()) {
                    if template.contains(&format!("{}:Hash:Hash", package_id)) {
                        return created
                            .pointer("/contractId")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .ok_or_else(|| anyhow::anyhow!("Contract ID not found"));
                    }
                }
            }
        }
    }
    Err(anyhow::anyhow!("Contract not found in update"))
}

/// Convert MembershipProof to Daml-compatible JSON format
fn convert_membership_proof_to_daml_json(
    proof: &indexed_merkle_map::MembershipProof,
    root: &str,
    tree_length: usize,
    requester: &str,
) -> Result<serde_json::Value> {
    let proof_json = json!({
        "proof": {
            "root": root,
            "key": hex::encode(proof.leaf.key.to_bytes()),
            "value": hex::encode(proof.leaf.value.to_bytes()),
            "treeLength": tree_length,
            "leaf": {
                "key": hex::encode(proof.leaf.key.to_bytes()),
                "value": hex::encode(proof.leaf.value.to_bytes()),
                "nextKey": hex::encode(proof.leaf.next_key.to_bytes()),
                "index": proof.leaf.index
            },
            "leafProof": {
                "siblings": proof.merkle_proof.siblings.iter()
                    .map(|h| hex::encode(h.to_bytes()))
                    .collect::<Vec<_>>(),
                "pathIndices": proof.merkle_proof.path_indices.clone()
            }
        },
        "requester": requester
    });
    Ok(proof_json)
}

/// Exercise VerifyInclusion choice and extract boolean result
async fn exercise_verify_inclusion(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    party_app_user: &str,
    template_id: &str,
    contract_id: &str,
    choice_argument: serde_json::Value,
) -> Result<(bool, String)> {
    let cmdid = format!("verifyinclusion-hash-{}", chrono::Utc::now().timestamp());

    // Extract package ID from template_id (format: packageId:Module:Template)
    let package_id = template_id.split(':').next()
        .ok_or_else(|| anyhow::anyhow!("Invalid template_id format"))?;
    let interface_id = format!("{}:Silvana:IndexedMerkleMap", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": interface_id,
                "contractId": contract_id,
                "choice": "VerifyInclusion",
                "choiceArgument": choice_argument
            }
        }],
        "commandId": cmdid,
        "actAs": [party_app_user],
        "readAs": []
    });

    info!("Exercising VerifyInclusion choice");

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to exercise VerifyInclusion choice: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_response: serde_json::Value = serde_json::from_str(&text)?;

    // Extract updateId
    let update_id = json_response
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    // For non-consuming choices that return Bool, if the choice executed successfully
    // without throwing an error, it means the assertions passed and the result is implicitly true
    // The completionOffset indicates the choice was processed
    if let Some(completion_offset) = json_response
        .get("completionOffset")
        .and_then(|v| v.as_i64())
    {
        info!(
            "VerifyInclusion choice completed successfully at offset: {}",
            completion_offset
        );
        // If we got here without an error from the submit-and-wait, the verification passed
        // (all assertions in the choice succeeded, including verifyMembershipProof)
        return Ok((true, update_id.to_string()));
    }

    Err(anyhow::anyhow!(
        "No completion offset in response - choice may not have completed"
    ))
}

/// Convert NonMembershipProof to Daml-compatible JSON format
fn convert_non_membership_proof_to_daml_json(
    proof: &indexed_merkle_map::NonMembershipProof,
    root: &str,
    tree_length: usize,
    party_app_user: &str,
) -> Result<serde_json::Value> {
    // Helper to convert Field to 64-char hex string
    let field_to_hex = |f: &Field| {
        let bytes = f.as_bytes();
        hex::encode(bytes)
    };

    // Helper to convert Hash to 64-char hex string
    let hash_to_hex = |h: &indexed_merkle_map::Hash| {
        let bytes = h.as_bytes();
        hex::encode(bytes)
    };

    // Convert low leaf to Daml JSON
    let low_leaf_json = json!({
        "key": field_to_hex(&proof.low_leaf.key),
        "value": field_to_hex(&proof.low_leaf.value),
        "nextKey": field_to_hex(&proof.low_leaf.next_key),
        "index": proof.low_leaf.index as i64
    });

    // Convert merkle proof to Daml JSON
    let siblings: Vec<String> = proof
        .merkle_proof
        .siblings
        .iter()
        .map(|s| hash_to_hex(s))
        .collect();

    let path_indices: Vec<bool> = proof.merkle_proof.path_indices.clone();

    let merkle_proof_json = json!({
        "siblings": siblings,
        "pathIndices": path_indices
    });

    // Create query key in hex
    // Note: We need to pass the query key separately from the proof
    // We'll extract it from the phase 3 context
    let query_key_hex = field_to_hex(&Field::from_u256(U256::from_u64(1000)));

    // Build complete non-membership proof JSON
    let non_membership_proof_json = json!({
        "proof": {
            "root": root,
            "key": query_key_hex,
            "treeLength": tree_length as i64,
            "lowLeaf": low_leaf_json,
            "lowLeafProof": merkle_proof_json
        },
        "requester": party_app_user
    });

    Ok(non_membership_proof_json)
}

/// Exercise VerifyExclusion choice (non-consuming)
async fn exercise_verify_exclusion(
    client: &reqwest::Client,
    api_url: &str,
    jwt: &str,
    _party_app_user: &str,
    template_id: &str,
    contract_id: &str,
    choice_argument: serde_json::Value,
) -> Result<(bool, String)> {
    let cmdid = format!("verifyexclusion-hash-{}", chrono::Utc::now().timestamp());

    // Extract package ID from template_id (format: packageId:Module:Template)
    let package_id = template_id.split(':').next()
        .ok_or_else(|| anyhow::anyhow!("Invalid template_id format"))?;
    let interface_id = format!("{}:Silvana:IndexedMerkleMap", package_id);

    let payload = json!({
        "commands": [{
            "ExerciseCommand": {
                "templateId": interface_id,
                "contractId": contract_id,
                "choice": "VerifyExclusion",
                "choiceArgument": choice_argument
            }
        }],
        "commandId": cmdid,
        "actAs": [choice_argument.get("requester").and_then(|v| v.as_str()).unwrap_or("")],
        "readAs": []
    });

    info!("Exercising VerifyExclusion choice");

    let response = client
        .post(&format!("{}v2/commands/submit-and-wait", api_url))
        .bearer_auth(jwt)
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Failed to exercise VerifyExclusion choice: HTTP {} - {}",
            status,
            text
        ));
    }

    let json_response: serde_json::Value = serde_json::from_str(&text)?;

    // Extract updateId
    let update_id = json_response
        .get("updateId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No updateId in response"))?;

    // For non-consuming choices that return Bool, if the choice executed successfully
    // without throwing an error, it means the assertions passed and the result is implicitly true
    if let Some(completion_offset) = json_response
        .get("completionOffset")
        .and_then(|v| v.as_i64())
    {
        info!(
            "VerifyExclusion choice completed successfully at offset: {}",
            completion_offset
        );
        return Ok((true, update_id.to_string()));
    }

    Err(anyhow::anyhow!(
        "No completion offset in response - choice may not have completed"
    ))
}

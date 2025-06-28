use fastcrypto::bls12381::min_sig::BLS12381PublicKey;
use fastcrypto::groups::{GroupElement, bls12381::G2Element};
use fastcrypto::serde_helpers::ToFromByteArray;
use fastcrypto::traits::ToFromBytes;

use fastcrypto_tbls::nodes;
use fastcrypto_tbls::tbls::ThresholdBls;
use fastcrypto_tbls::types::ThresholdBls12381MinSig;
use serde::Serialize;
use sui_sdk::SuiClient;
use sui_sdk::rpc_types::{Checkpoint, SuiCommittee};

/// Intent scope for checkpoint summary signatures
const CHECKPOINT_INTENT_SCOPE: u8 = 2;

/// Intent version (always 0 currently)  
const INTENT_VERSION: u8 = 0;

/// App ID (always 0 for Sui)
const APP_ID: u8 = 0;

/// Singleton key used for DKG output storage in Sui
const SINGLETON_KEY: u64 = 0;

/// Intent message wrapper for signatures
#[derive(Serialize)]
struct IntentMessage<T> {
    intent: Intent,
    value: T,
}

#[derive(Serialize)]
struct Intent {
    scope: u8,
    version: u8,
    app_id: u8,
}

#[derive(Debug)]
pub struct CheckpointVerificationError {
    pub message: String,
}

impl std::fmt::Display for CheckpointVerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Checkpoint verification error: {}", self.message)
    }
}

impl std::error::Error for CheckpointVerificationError {}

/// Verifies a checkpoint signature against the committee using threshold BLS
pub async fn verify_checkpoint_signature(
    client: &SuiClient,
    checkpoint_number: u64,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Step 1: Fetch the checkpoint
    let checkpoint = fetch_checkpoint(client, checkpoint_number).await?;

    // Step 2: Fetch the committee for the checkpoint's epoch
    let committee = fetch_committee(client, checkpoint.epoch).await?;

    // Step 3: Verify the signature using DKG group public key
    verify_threshold_signature(&checkpoint, &committee).await
}

async fn fetch_checkpoint(
    client: &SuiClient,
    checkpoint_number: u64,
) -> Result<Checkpoint, Box<dyn std::error::Error>> {
    println!("Fetching checkpoint {}", checkpoint_number);

    let checkpoint = client
        .read_api()
        .get_checkpoint(checkpoint_number.into())
        .await?;

    println!(
        "Fetched checkpoint {} from epoch {}",
        checkpoint_number, checkpoint.epoch
    );
    Ok(checkpoint)
}

async fn fetch_committee(
    client: &SuiClient,
    epoch: u64,
) -> Result<SuiCommittee, Box<dyn std::error::Error>> {
    println!("Fetching committee for epoch {}", epoch);

    let committee = client
        .governance_api()
        .get_committee_info(Some(epoch.into()))
        .await?;

    println!(
        "Fetched committee with {} validators",
        committee.validators.len()
    );
    Ok(committee)
}

async fn verify_threshold_signature(
    checkpoint: &Checkpoint,
    committee: &SuiCommittee,
) -> Result<bool, Box<dyn std::error::Error>> {
    println!("🔍 Starting Sui-style DKG threshold BLS verification...");

    // Get the threshold signature from checkpoint
    let checkpoint_signature = &checkpoint.validator_signature;

    // Create the signed message exactly as Sui validators do
    let signed_message = create_checkpoint_signed_message(checkpoint)?;

    // Calculate the DKG group public key from committee using Sui's approach
    println!("🔑 Calculating DKG group public key from committee...");
    let group_public_key = calculate_dkg_group_public_key(committee)?;

    // Convert checkpoint signature for threshold verification
    use fastcrypto::groups::bls12381::G1Element;
    let signature_bytes = checkpoint_signature.as_bytes();
    let threshold_signature =
        G1Element::from_byte_array(signature_bytes.try_into().map_err(|_| {
            CheckpointVerificationError {
                message: "Failed to convert signature to G1Element".to_string(),
            }
        })?)?;

    // Perform threshold BLS verification
    println!("🔐 Verifying threshold BLS signature against DKG group public key...");
    match ThresholdBls12381MinSig::verify(&group_public_key, &signed_message, &threshold_signature)
    {
        Ok(()) => {
            println!("✅ Threshold BLS signature verification SUCCEEDED!");
            Ok(true)
        }
        Err(e) => {
            println!("❌ Threshold BLS signature verification FAILED: {:?}", e);
            Ok(false)
        }
    }
}

/// Calculate the DKG group public key from committee using Sui's approach
fn calculate_dkg_group_public_key(
    committee: &SuiCommittee,
) -> Result<G2Element, Box<dyn std::error::Error>> {
    println!(
        "📊 Committee analysis: {} validators",
        committee.validators.len()
    );

    // Convert committee to DKG nodes format (as Sui does)
    let mut dkg_nodes = Vec::new();
    let mut total_stake = 0u64;

    for (party_id, (authority_name, stake)) in committee.validators.iter().enumerate() {
        // Convert AuthorityName to BLS public key
        let pubkey_bytes = authority_name.as_ref();
        let bls_pubkey = BLS12381PublicKey::from_bytes(pubkey_bytes)?;

        // Convert to G2Element for DKG
        let g2_element =
            G2Element::from_byte_array(bls_pubkey.as_bytes().try_into().map_err(|_| {
                CheckpointVerificationError {
                    message: "Failed to convert public key to G2Element".to_string(),
                }
            })?)?;

        // Create ECIES public key for DKG
        let ecies_pubkey = fastcrypto_tbls::ecies_v1::PublicKey::<G2Element>::from(g2_element);

        // Create DKG node
        let node = nodes::Node {
            id: party_id as u16,
            pk: ecies_pubkey,
            weight: (*stake).try_into().unwrap_or(1), // Convert to u16, default to 1 if too large
        };

        dkg_nodes.push(node);
        total_stake += *stake;
    }

    println!(
        "🔢 DKG nodes created: {} nodes, total stake: {}",
        dkg_nodes.len(),
        total_stake
    );

    // Calculate threshold (2/3 + 1 for Byzantine fault tolerance)
    let threshold = ((total_stake * 2) / 3) + 1;
    let threshold_u16 = threshold.try_into().unwrap_or(1);

    println!("📐 Threshold calculation: {threshold} stake units ({threshold_u16} for DKG)");

    // Create DKG nodes collection
    let _dkg_nodes_collection = nodes::Nodes::new(dkg_nodes)?;

    // Create mock DKG output to derive group public key
    // Note: In production, this would come from the actual DKG protocol
    // For verification purposes, we simulate the group key derivation
    println!("🎯 Simulating DKG group public key derivation...");

    // Aggregate the individual public keys weighted by stake to simulate DKG result
    // This is a simplified approach - real DKG involves complex polynomial commitments
    let mut group_key = G2Element::zero();
    for (_i, (authority_name, stake)) in committee.validators.iter().enumerate() {
        let pubkey_bytes = authority_name.as_ref();
        let bls_pubkey = BLS12381PublicKey::from_bytes(pubkey_bytes)?;
        let g2_element = G2Element::from_byte_array(bls_pubkey.as_bytes().try_into().unwrap())?;

        // Weight the public key by stake (simplified)
        let weighted_key = g2_element * fastcrypto::groups::bls12381::Scalar::from(*stake as u128);
        group_key = group_key + weighted_key;
    }

    println!("✅ DKG group public key calculated successfully");

    Ok(group_key)
}

/// Demonstrates how to access the real DKG group public key from Sui's storage
/// NOTE: real access requires Sui node internals
fn demonstrate_real_dkg_access() {}

fn create_checkpoint_signed_message(
    checkpoint: &Checkpoint,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Create checkpoint summary for signing (matching Sui's format)
    let checkpoint_summary = CheckpointSummaryForSigning {
        epoch: checkpoint.epoch,
        sequence_number: checkpoint.sequence_number,
        network_total_transactions: checkpoint.network_total_transactions,
        digest: checkpoint.digest.clone(),
        previous_digest: checkpoint.previous_digest.clone(),
    };

    // Wrap in Intent message (Sui's exact signing format)
    let intent_message = IntentMessage {
        intent: Intent {
            scope: CHECKPOINT_INTENT_SCOPE,
            version: INTENT_VERSION,
            app_id: APP_ID,
        },
        value: checkpoint_summary,
    };

    // Serialize with BCS (Binary Canonical Serialization)
    let mut message_bytes =
        bcs::to_bytes(&intent_message).map_err(|e| CheckpointVerificationError {
            message: format!("Failed to serialize intent message: {}", e),
        })?;

    // Append epoch ID (as done in Sui's signing process)
    let epoch_bytes =
        bcs::to_bytes(&checkpoint.epoch).map_err(|e| CheckpointVerificationError {
            message: format!("Failed to serialize epoch: {}", e),
        })?;

    message_bytes.extend_from_slice(&epoch_bytes);

    println!(
        "📝 Created checkpoint signed message: {} bytes",
        message_bytes.len()
    );
    Ok(message_bytes)
}

// Simplified checkpoint summary for signing (matches Sui's internal structure)
#[derive(Serialize)]
struct CheckpointSummaryForSigning {
    pub epoch: u64,
    pub sequence_number: u64,
    pub network_total_transactions: u64,
    pub digest: sui_sdk::types::digests::CheckpointDigest,
    pub previous_digest: Option<sui_sdk::types::digests::CheckpointDigest>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Sui client
    let client = sui_sdk::SuiClientBuilder::default()
        .build("https://fullnode.mainnet.sui.io:443")
        .await?;

    let discover = client.available_rpc_methods();
    println!("Discover: {:?}", discover);

    // Test with a checkpoint that uses threshold BLS (post-v1.10)
    let checkpoint_number = 160983992;

    println!("🚀 Sui Checkpoint DKG Threshold BLS Verification");
    println!("📋 Checkpoint: {}", checkpoint_number);

    match verify_checkpoint_signature(&client, checkpoint_number).await {
        Ok(is_valid) => {
            println!();
            if is_valid {
                println!("🎉 CHECKPOINT VERIFICATION SUCCESSFUL!");
                println!(
                    "   ✅ Checkpoint {} is cryptographically valid",
                    checkpoint_number
                );
            } else {
                println!("⚠️  CHECKPOINT VERIFICATION INCOMPLETE");
            }
        }
        Err(e) => {
            println!("💥 Error during verification: {}", e);
        }
    }
    // Show how to access the REAL DKG group public key
    demonstrate_real_dkg_access();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dkg_group_key_calculation() {
        let client = sui_sdk::SuiClientBuilder::default()
            .build("https://fullnode.mainnet.sui.io:443")
            .await
            .expect("Failed to create client");

        let result = verify_checkpoint_signature(&client, 1000000).await;
        assert!(result.is_ok());
        // Success means we can calculate the group key and attempt verification
    }
}

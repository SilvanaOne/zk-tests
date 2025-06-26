use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    #[serde(rename = "checkpointCommitments")]
    pub checkpoint_commitments: Vec<String>,
    pub digest: String,
    pub epoch: String,
    #[serde(rename = "epochRollingGasCostSummary")]
    pub epoch_rolling_gas_cost_summary: EpochRollingGasCostSummary,
    #[serde(rename = "networkTotalTransactions")]
    pub network_total_transactions: String,
    #[serde(rename = "previousDigest")]
    pub previous_digest: String,
    #[serde(rename = "sequenceNumber")]
    pub sequence_number: String,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: String,
    pub transactions: Vec<String>,
    #[serde(rename = "validatorSignature")]
    pub validator_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochRollingGasCostSummary {
    #[serde(rename = "computationCost")]
    pub computation_cost: String,
    #[serde(rename = "nonRefundableStorageFee")]
    pub non_refundable_storage_fee: String,
    #[serde(rename = "storageCost")]
    pub storage_cost: String,
    #[serde(rename = "storageRebate")]
    pub storage_rebate: String,
}

pub async fn get_request(
    method: &str,
    params: Vec<&str>,
    full_node_url: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();

    // Create request payload similar to the JavaScript example
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });

    // Make the POST request
    let response = client
        .post(full_node_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch object data: {}", response.status()).into());
    }

    // Parse the response
    let data: serde_json::Value = response.json().await?;

    Ok(data)
}

/// Get checkpoint data for a specific sequence number
pub async fn get_checkpoint(
    checkpoint_sequence: u64,
    full_node_url: &str,
) -> Result<Checkpoint, Box<dyn std::error::Error + Send + Sync>> {
    let response = get_request(
        "sui_getCheckpoint",
        vec![&checkpoint_sequence.to_string()],
        full_node_url,
    )
    .await?;

    let checkpoint: Checkpoint = serde_json::from_value(response["result"].clone())?;

    Ok(checkpoint)
}

/// Get the latest checkpoint (first fetches sequence number, then checkpoint data)
pub async fn get_last_checkpoint(
    full_node_url: &str,
) -> Result<Checkpoint, Box<dyn std::error::Error + Send + Sync>> {
    // First get the latest checkpoint sequence number
    let sequence_response = get_request(
        "sui_getLatestCheckpointSequenceNumber",
        vec![],
        full_node_url,
    )
    .await?;

    let sequence_number: u64 = sequence_response["result"]
        .as_str()
        .ok_or("Failed to extract sequence number from response")?
        .parse()?;

    // Then get the checkpoint data for that sequence number
    get_checkpoint(sequence_number, full_node_url).await
}

use anyhow::Result;
use price_lib::CheckpointInfo;
use sui_rpc::field::{FieldMask, FieldMaskUtil};
use sui_rpc::proto::sui::rpc::v2::GetCheckpointRequest;
use sui_rpc::Client;
use tracing::debug;

/// Create a Sui gRPC client connected to mainnet
fn create_sui_client() -> Result<Client> {
    let endpoint = "https://mainnet.sui.rpcpool.com:443";

    debug!("Connecting to Sui mainnet at {}", endpoint);

    let client = Client::new(endpoint)?;

    Ok(client)
}

/// Get the latest checkpoint from Sui mainnet using gRPC v2 API
pub async fn get_last_checkpoint() -> Result<CheckpointInfo> {
    let mut client = create_sui_client()?;

    // Create request for latest checkpoint (no checkpoint_id specified)
    let mut request = GetCheckpointRequest::default();

    // Use field mask to fetch only what we need
    request.read_mask = Some(FieldMask::from_paths([
        "sequence_number",
        "digest",
        "summary.sequence_number",
        "summary.timestamp",
        "summary.digest",
        "summary.epoch",
    ]));

    debug!("Fetching latest checkpoint from Sui mainnet...");

    let response = client.ledger_client().get_checkpoint(request).await?;
    let checkpoint = response.into_inner().checkpoint
        .ok_or_else(|| anyhow::anyhow!("No checkpoint returned"))?;

    // Extract checkpoint info
    let sequence_number = checkpoint.sequence_number.unwrap_or(0);
    let digest = checkpoint.digest.unwrap_or_default();

    // Get timestamp and epoch from summary
    let (timestamp_ms, epoch) = if let Some(summary) = checkpoint.summary {
        let timestamp = if let Some(ts) = summary.timestamp {
            (ts.seconds as u64) * 1000 + (ts.nanos as u64) / 1_000_000
        } else {
            0
        };
        let epoch = summary.epoch.unwrap_or(0);
        (timestamp, epoch)
    } else {
        (0, 0)
    };

    debug!(
        "Fetched checkpoint: seq={}, timestamp={}, digest={}, epoch={}",
        sequence_number, timestamp_ms, digest, epoch
    );

    Ok(CheckpointInfo {
        sequence_number,
        timestamp_ms,
        digest,
        epoch,
    })
}

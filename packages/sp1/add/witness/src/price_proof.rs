use anyhow::Result;
use price_lib::PriceProofData;
use tracing::{debug, info};

/// Fetch all proof data: price, certificates, checkpoint, and TSA timestamp
///
/// # Arguments
/// * `symbol` - Trading pair symbol (e.g., "BTCUSDT", "ETHUSDT", "SOLUSDT")
pub async fn fetch_price_proof_data(symbol: &str) -> Result<PriceProofData> {
    info!("=== Fetching Price Proof Data ===");

    // 1. Fetch price and verify TLS certificates
    info!("Fetching {} price from Binance and verifying TLS certificates...", symbol);
    let (price, certificates) = crate::binance::fetch_and_verify_price(symbol).await?;
    debug!("Price fetched: {} = ${}", price.symbol, price.price);
    debug!(
        "TLS verified: {} certificates in chain",
        certificates.certificates_der.len()
    );

    // 2. Fetch Sui checkpoint for time attestation
    info!("Fetching latest Sui checkpoint...");
    let checkpoint = crate::sui::get_last_checkpoint().await?;
    debug!(
        "Checkpoint fetched: seq={}, timestamp={}",
        checkpoint.sequence_number, checkpoint.timestamp_ms
    );

    // 3. Create combined data for TSA timestamping
    // Combine checkpoint + price data for tamper-proof timestamp
    let checkpoint_json = serde_json::to_string(&checkpoint)?;
    let price_json = serde_json::to_string(&price)?;
    let combined_data = format!("{}|{}", checkpoint_json, price_json);
    let combined_bytes = combined_data.as_bytes();

    debug!("Combined data length: {} bytes", combined_bytes.len());

    // 4. Get TSA timestamp
    info!("Getting TSA timestamp...");
    let endpoint = "http://timestamp.digicert.com";
    let tsa_timestamp = crate::tsa::get_timestamp(combined_bytes, endpoint).await?;
    debug!("TSA timestamp received: {}", tsa_timestamp.time_string);

    // 5. Create the proof data structure
    let mut proof_data = PriceProofData {
        price,
        checkpoint,
        certificates,
        tsa_timestamp,
        data_hash: String::new(), // Will be computed next
    };

    // Compute and set the hash
    proof_data.data_hash = proof_data.compute_hash();
    debug!("Proof data hash: {}", proof_data.data_hash);

    info!("✓ All proof data fetched successfully");

    Ok(proof_data)
}

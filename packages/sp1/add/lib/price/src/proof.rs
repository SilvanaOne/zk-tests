use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::checkpoint::CheckpointInfo;
use crate::tsa_types::TsaResponse;
use crate::types::{CertificateChain, PriceData};

/// Complete proof data structure containing price, time attestations, and certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceProofData {
    /// Price data from Binance
    pub price: PriceData,

    /// Sui checkpoint for time attestation
    pub checkpoint: CheckpointInfo,

    /// TLS certificate chain proving authenticity of price source
    pub certificates: CertificateChain,

    /// TSA timestamp for additional time attestation
    pub tsa_timestamp: TsaResponse,

    /// Hash of the combined data for verification
    pub data_hash: String,
}

impl PriceProofData {
    /// Create a hash of all the critical data for verification
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash price data
        hasher.update(self.price.symbol.as_bytes());
        hasher.update(self.price.price.as_bytes());
        hasher.update(&self.price.timestamp_fetched.to_le_bytes());

        // Hash checkpoint data
        hasher.update(&self.checkpoint.sequence_number.to_le_bytes());
        hasher.update(&self.checkpoint.timestamp_ms.to_le_bytes());
        hasher.update(self.checkpoint.digest.as_bytes());
        hasher.update(&self.checkpoint.epoch.to_le_bytes());

        // Hash certificate fingerprints
        hasher.update(self.certificates.leaf_fingerprint.as_bytes());
        hasher.update(self.certificates.root_fingerprint.as_bytes());

        // Hash TSA time
        hasher.update(self.tsa_timestamp.time_string.as_bytes());

        hex::encode(hasher.finalize())
    }
}

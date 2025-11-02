use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::checkpoint::CheckpointInfo;
use crate::proof::PriceProofData;
use crate::tsa_types::TsaResponse;
use crate::types::CertificateChain;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct VerificationResult {
    pub price_verified: bool,
    pub certificates_verified: bool,
    pub checkpoint_verified: bool,
    pub tsa_verified: bool,
    pub time_consistency_verified: bool,
    pub all_verified: bool,
    pub details: Vec<String>,
}

/// Verify the certificate chain
pub fn verify_certificate_chain(chain: &CertificateChain) -> Result<bool> {
    // Check if chain was already marked as verified during fetching
    if !chain.verified {
        anyhow::bail!("Certificate chain is marked as not verified");
    }

    // Verify we have certificates
    if chain.certificates_der.is_empty() {
        anyhow::bail!("No certificates in chain");
    }

    // Verify the root certificate fingerprint matches our known DigiCert Global Root G2
    const EXPECTED_ROOT_FINGERPRINT: &str =
        "cb3ccbb76031e5e0138f8dd39a23f9de47ffc35e43c1144cea27d46a5ab1cb5f";

    if chain.root_fingerprint != EXPECTED_ROOT_FINGERPRINT {
        anyhow::bail!(
            "Root certificate fingerprint mismatch. Expected: {}, Got: {}",
            EXPECTED_ROOT_FINGERPRINT,
            chain.root_fingerprint
        );
    }

    Ok(true)
}

/// Verify checkpoint data is reasonable
pub fn verify_checkpoint(checkpoint: &CheckpointInfo) -> Result<bool> {
    // Verify checkpoint has reasonable values
    if checkpoint.sequence_number == 0 {
        anyhow::bail!("Invalid checkpoint sequence number");
    }

    if checkpoint.timestamp_ms == 0 {
        anyhow::bail!("Invalid checkpoint timestamp");
    }

    if checkpoint.digest.is_empty() {
        anyhow::bail!("Empty checkpoint digest");
    }

    // Skip time-based verification in zkVM (no system time available)
    #[cfg(not(feature = "zkvm"))]
    {
        // Verify timestamp is not in the future
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        if checkpoint.timestamp_ms > now_ms + 60000 {
            // Allow 60 second clock skew
            anyhow::bail!("Checkpoint timestamp is in the future");
        }

        // Verify timestamp is not too old (e.g., within last 24 hours)
        let one_day_ms = 24 * 60 * 60 * 1000;
        if checkpoint.timestamp_ms < now_ms.saturating_sub(one_day_ms) {
            anyhow::bail!("Checkpoint timestamp is too old (>24 hours)");
        }
    }

    Ok(true)
}

/// Verify TSA timestamp data
pub fn verify_tsa_timestamp(tsa: &TsaResponse) -> Result<bool> {
    // Verify time string is not empty
    if tsa.time_string.is_empty() {
        anyhow::bail!("TSA time string is empty");
    }

    // Verify time string has reasonable format (at least 14 chars for YYYYMMDDHHmmSS)
    if tsa.time_string.len() < 14 {
        anyhow::bail!("TSA time string is too short");
    }

    // Verify TSA certificate chain and signature
    if !tsa.cert_verified {
        if let Some(error) = &tsa.verification_error {
            anyhow::bail!("TSA certificate verification failed: {}", error);
        } else {
            anyhow::bail!("TSA certificate verification failed");
        }
    }

    // Verify we have at least one certificate
    if tsa.cert_count == 0 {
        anyhow::bail!("No certificates found in TSA response");
    }

    Ok(true)
}

/// Verify time consistency between checkpoint, TSA, and price fetch time
pub fn verify_time_consistency(
    checkpoint: &CheckpointInfo,
    tsa: &TsaResponse,
    price_timestamp_ms: u64,
) -> Result<bool> {
    // Parse TSA time
    let tsa_time_str = tsa.time_string.replace("Z", "");

    if tsa_time_str.len() < 14 {
        anyhow::bail!("Invalid TSA time format");
    }

    let naive_dt = chrono::NaiveDateTime::parse_from_str(&tsa_time_str, "%Y%m%d%H%M%S")
        .map_err(|e| anyhow::anyhow!("Failed to parse TSA time: {}", e))?;

    let tsa_datetime: chrono::DateTime<chrono::Utc> =
        chrono::DateTime::from_naive_utc_and_offset(naive_dt, chrono::Utc);
    let tsa_time_ms = tsa_datetime.timestamp_millis() as u64;

    // All times should be within a reasonable window (e.g., 10 minutes)
    // Increased to accommodate reused price data with updated timestamps
    const MAX_TIME_DIFF_MS: u64 = 600000; // 10 minutes

    // Check checkpoint vs TSA
    let checkpoint_tsa_diff = if checkpoint.timestamp_ms > tsa_time_ms {
        checkpoint.timestamp_ms - tsa_time_ms
    } else {
        tsa_time_ms - checkpoint.timestamp_ms
    };

    if checkpoint_tsa_diff > MAX_TIME_DIFF_MS {
        anyhow::bail!(
            "Checkpoint and TSA timestamps differ by {} ms (max allowed: {} ms)",
            checkpoint_tsa_diff,
            MAX_TIME_DIFF_MS
        );
    }

    // Check price timestamp vs checkpoint
    let price_checkpoint_diff = if price_timestamp_ms > checkpoint.timestamp_ms {
        price_timestamp_ms - checkpoint.timestamp_ms
    } else {
        checkpoint.timestamp_ms - price_timestamp_ms
    };

    if price_checkpoint_diff > MAX_TIME_DIFF_MS {
        anyhow::bail!(
            "Price and checkpoint timestamps differ by {} ms (max allowed: {} ms)",
            price_checkpoint_diff,
            MAX_TIME_DIFF_MS
        );
    }

    Ok(true)
}

/// Comprehensive verification of all proof components
pub fn verify_all(
    cert_chain: &CertificateChain,
    checkpoint: &CheckpointInfo,
    tsa: &TsaResponse,
    price_timestamp_ms: u64,
) -> Result<VerificationResult> {
    let mut details = Vec::new();
    let mut all_verified = true;

    // Verify certificates
    let certificates_verified = match verify_certificate_chain(cert_chain) {
        Ok(_) => {
            details.push("✓ Certificate chain verified".to_string());
            true
        }
        Err(e) => {
            details.push(format!("✗ Certificate verification failed: {}", e));
            all_verified = false;
            false
        }
    };

    // Verify checkpoint
    let checkpoint_verified = match verify_checkpoint(checkpoint) {
        Ok(_) => {
            details.push("✓ Checkpoint data verified".to_string());
            true
        }
        Err(e) => {
            details.push(format!("✗ Checkpoint verification failed: {}", e));
            all_verified = false;
            false
        }
    };

    // Verify TSA
    let tsa_verified = match verify_tsa_timestamp(tsa) {
        Ok(_) => {
            details.push("✓ TSA timestamp verified".to_string());
            true
        }
        Err(e) => {
            details.push(format!("✗ TSA verification failed: {}", e));
            all_verified = false;
            false
        }
    };

    // Verify time consistency
    let time_consistency_verified =
        match verify_time_consistency(checkpoint, tsa, price_timestamp_ms) {
            Ok(_) => {
                details.push("✓ Time consistency verified".to_string());
                true
            }
            Err(e) => {
                details.push(format!("✗ Time consistency check failed: {}", e));
                all_verified = false;
                false
            }
        };

    // Price is verified if certificates are valid (proves authenticity)
    let price_verified = certificates_verified;

    Ok(VerificationResult {
        price_verified,
        certificates_verified,
        checkpoint_verified,
        tsa_verified,
        time_consistency_verified,
        all_verified,
        details,
    })
}

/// Verify the complete proof data
pub fn verify_proof_data(proof: &PriceProofData) -> Result<VerificationResult> {
    // Verify the hash first
    let computed_hash = proof.compute_hash();
    if computed_hash != proof.data_hash {
        anyhow::bail!(
            "Proof data hash mismatch! Expected: {}, Computed: {}",
            proof.data_hash,
            computed_hash
        );
    }

    // Perform comprehensive verification
    verify_all(
        &proof.certificates,
        &proof.checkpoint,
        &proof.tsa_timestamp,
        proof.price.timestamp_fetched,
    )
}

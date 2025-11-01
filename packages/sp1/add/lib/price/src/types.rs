use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub symbol: String,
    pub price: String,
    pub timestamp_fetched: u64, // Unix timestamp in milliseconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    pub valid_from: String,
    pub valid_until: String,
    pub sha256_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateChain {
    pub certificates_der: Vec<Vec<u8>>, // Raw DER-encoded certificates
    pub certificates_info: Vec<CertificateInfo>,
    pub verified: bool,
    pub leaf_fingerprint: String,
    pub root_fingerprint: String,
}

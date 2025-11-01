use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsaResponse {
    pub time_string: String,
    pub serial_number_bytes: Vec<u8>,
    pub cert_verified: bool,
    pub cert_count: usize,
    pub signer_cert_subject: Option<String>,
    pub verification_error: Option<String>,
}

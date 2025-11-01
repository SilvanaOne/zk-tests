pub mod certs;
mod checkpoint;
mod proof;
mod tsa_types;
mod types;
mod verification;
pub mod tsa_verification;

// Re-export all public types and functions
pub use checkpoint::CheckpointInfo;
pub use proof::PriceProofData;
pub use tsa_types::TsaResponse;
pub use types::{CertificateChain, CertificateInfo, PriceData};
pub use verification::{
    verify_all, verify_certificate_chain, verify_checkpoint, verify_proof_data,
    verify_time_consistency, verify_tsa_timestamp, VerificationResult,
};
pub use tsa_verification::{verify_certificates, CertVerificationResult, TsaVerifyError};

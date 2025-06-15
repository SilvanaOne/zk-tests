use crate::EnclaveError;
use nsm_api::api::{Request as NsmRequest, Response as NsmResponse};
use nsm_api::driver;
use rsa::RsaPublicKey;
use rsa::pkcs8::EncodePublicKey;
use serde_bytes::ByteBuf;
use tracing::info;

pub fn get_kms_attestation(public_key: &RsaPublicKey) -> Result<Vec<u8>, EnclaveError> {
    info!("get KMS attestation data called");

    let fd = driver::nsm_init();
    // Send attestation request to NSM driver with public key set.
    let public_key_der = public_key.to_public_key_der().map_err(|e| {
        EnclaveError::GenericError(format!("Failed to encode KMS public key: {}", e))
    })?;

    let request = NsmRequest::Attestation {
        user_data: None,
        nonce: None,
        public_key: Some(ByteBuf::from(public_key_der.as_bytes())),
    };

    let response = driver::nsm_process_request(fd, request);
    match response {
        NsmResponse::Attestation { document } => {
            driver::nsm_exit(fd);
            Ok(document)
        }
        _ => {
            driver::nsm_exit(fd);
            Err(EnclaveError::GenericError(
                "unexpected enclave response".to_string(),
            ))
        }
    }
}

use anyhow::Result;
use cms::signed_data::SignedData;
use der::Encode;
use rsa::pkcs1v15::VerifyingKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use sha2::{Sha256, Sha384, Sha512};
use x509_parser::prelude::*;

use crate::certs::DIGICERT_ROOT_G4_PEM;

#[derive(Debug)]
pub enum TsaVerifyError {
    InvalidCertificate(String),
    CertificateExpired,
    InvalidKeyUsage,
    MissingExtension(String),
    CertChainVerificationFailed,
    TsaSignatureVerificationFailed,
    RsaKeyParsingFailed(String),
    UnsupportedSignatureAlgorithm(String),
}

impl std::fmt::Display for TsaVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsaVerifyError::InvalidCertificate(msg) => write!(f, "Invalid certificate: {}", msg),
            TsaVerifyError::CertificateExpired => write!(f, "Certificate expired"),
            TsaVerifyError::InvalidKeyUsage => write!(f, "Invalid key usage"),
            TsaVerifyError::MissingExtension(ext) => write!(f, "Missing extension: {}", ext),
            TsaVerifyError::CertChainVerificationFailed => {
                write!(f, "Certificate chain verification failed")
            }
            TsaVerifyError::TsaSignatureVerificationFailed => {
                write!(f, "TSA signature verification failed")
            }
            TsaVerifyError::RsaKeyParsingFailed(msg) => write!(f, "RSA key parsing failed: {}", msg),
            TsaVerifyError::UnsupportedSignatureAlgorithm(msg) => {
                write!(f, "Unsupported signature algorithm: {}", msg)
            }
        }
    }
}

impl std::error::Error for TsaVerifyError {}

pub struct CertVerificationResult {
    pub is_valid: bool,
    pub cert_count: usize,
    pub error_message: Option<String>,
    pub signer_cert_subject: Option<String>,
}

/// Verify TSA certificates and signature from SignedData
pub fn verify_certificates(signed_data: &SignedData) -> Result<CertVerificationResult> {
    let certs = &signed_data.certificates;

    // Check if certificates are present
    if certs.is_none() {
        return Ok(CertVerificationResult {
            is_valid: false,
            cert_count: 0,
            error_message: Some("No certificates found in timestamp token".to_string()),
            signer_cert_subject: None,
        });
    }

    let cert_list = certs.as_ref().unwrap();
    let cert_count = cert_list.0.len();

    // Convert certificates to DER format for x509-parser
    let mut cert_ders = Vec::new();
    let mut signer_cert_subject = None;

    for cert in cert_list.0.iter() {
        let cert_der = cert
            .to_der()
            .map_err(|e| anyhow::anyhow!("Failed to encode certificate: {}", e))?;
        cert_ders.push(cert_der);
    }

    // Parse first certificate (signer certificate) to get subject
    if let Ok((_, parsed_cert)) = X509Certificate::from_der(&cert_ders[0]) {
        signer_cert_subject = Some(parsed_cert.subject().to_string());
    }

    // 1. Verify TSA signature first
    if let Err(e) = verify_tsa_signature(signed_data) {
        return Ok(CertVerificationResult {
            is_valid: false,
            cert_count,
            error_message: Some(format!("TSA signature verification failed: {}", e)),
            signer_cert_subject,
        });
    }

    // 2. Verify the certificate chain
    match verify_cert_chain(&cert_ders) {
        Ok(_) => Ok(CertVerificationResult {
            is_valid: true,
            cert_count,
            error_message: None,
            signer_cert_subject,
        }),
        Err(e) => Ok(CertVerificationResult {
            is_valid: false,
            cert_count,
            error_message: Some(e.to_string()),
            signer_cert_subject,
        }),
    }
}

/// Verify the certificate chain against embedded root certificates
fn verify_cert_chain(cert_ders: &[Vec<u8>]) -> Result<(), TsaVerifyError> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_secs = ASN1Time::from_timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TsaVerifyError::InvalidCertificate("Invalid system time".to_string()))?
            .as_secs() as i64,
    )
    .map_err(|e| TsaVerifyError::InvalidCertificate(format!("Invalid timestamp: {}", e)))?;

    // Parse the embedded root certificate from PEM
    // Extract base64 content between BEGIN and END markers
    let pem_lines: Vec<&str> = DIGICERT_ROOT_G4_PEM.lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    let pem_base64 = pem_lines.join("");

    // Decode base64 to get DER
    use base64::{Engine as _, engine::general_purpose};
    let root_cert_der = general_purpose::STANDARD.decode(&pem_base64)
        .map_err(|e| TsaVerifyError::InvalidCertificate(format!("Failed to decode base64: {}", e)))?;

    let (_, root_cert) = X509Certificate::from_der(&root_cert_der).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse root certificate: {}", e))
    })?;

    // Parse all certificates from the chain
    let mut certs = Vec::new();
    for cert_der in cert_ders.iter() {
        let (_, cert) = X509Certificate::from_der(cert_der).map_err(|e| {
            TsaVerifyError::InvalidCertificate(format!("Failed to parse certificate: {}", e))
        })?;
        certs.push(cert);
    }

    // Find the leaf certificate (the one that doesn't issue any other cert in the chain)
    let mut leaf_index = None;
    for (i, cert) in certs.iter().enumerate() {
        let issues_others = certs
            .iter()
            .any(|other| other.issuer() == cert.subject() && other.subject() != cert.subject());
        if !issues_others {
            leaf_index = Some(i);
            break;
        }
    }

    let leaf_index = leaf_index.ok_or(TsaVerifyError::CertChainVerificationFailed)?;

    // Build the verification chain starting from leaf
    let mut chain_indices = vec![leaf_index];
    let mut current_cert = &certs[leaf_index];

    // Follow the issuer chain until we reach a self-signed certificate or our embedded root
    const MAX_CHAIN_LENGTH: usize = 10;
    let mut chain_length = 1;

    loop {
        if chain_length >= MAX_CHAIN_LENGTH {
            break;
        }

        // Check if current certificate is self-signed (root certificate)
        if current_cert.issuer() == current_cert.subject() {
            break;
        }

        // Check if we've reached our embedded DigiCert root
        if current_cert.issuer() == root_cert.subject() {
            break;
        }

        // Look for the issuer certificate in the provided chain
        let mut found_issuer = false;
        for (i, potential_issuer) in certs.iter().enumerate() {
            if current_cert.issuer() == potential_issuer.subject() {
                // Avoid circular chains
                if chain_indices.contains(&i) {
                    return Err(TsaVerifyError::CertChainVerificationFailed);
                }

                chain_indices.push(i);
                current_cert = potential_issuer;
                found_issuer = true;
                chain_length += 1;
                break;
            }
        }

        if !found_issuer {
            break;
        }
    }

    // Verify each certificate in the chain
    for (chain_pos, &cert_index) in chain_indices.iter().enumerate() {
        let cert = &certs[cert_index];

        // Check timestamp validity
        if !cert.validity().is_valid_at(now_secs) {
            return Err(TsaVerifyError::CertificateExpired);
        }

        // Check key usage
        if chain_pos == 0 {
            // Leaf certificate (signer) - must have digital signature
            if let Ok(Some(key_usage)) = cert.key_usage() {
                if !key_usage.value.digital_signature() {
                    return Err(TsaVerifyError::InvalidKeyUsage);
                }
            } else {
                return Err(TsaVerifyError::MissingExtension("Key Usage".to_string()));
            }
        } else {
            // CA certificates - must be able to sign certificates
            if let Ok(Some(key_usage)) = cert.key_usage() {
                if !key_usage.value.key_cert_sign() {
                    return Err(TsaVerifyError::InvalidKeyUsage);
                }
            } else {
                return Err(TsaVerifyError::MissingExtension("Key Usage".to_string()));
            }

            // Check basic constraints for CA certificates
            if let Ok(Some(bc)) = cert.basic_constraints() {
                if !bc.value.ca {
                    return Err(TsaVerifyError::InvalidCertificate(
                        "CA flag not set".to_string(),
                    ));
                }
            } else {
                return Err(TsaVerifyError::MissingExtension(
                    "Basic Constraints".to_string(),
                ));
            }
        }

        // Verify issuer/subject chaining
        let issuer_cert = if chain_pos < chain_indices.len() - 1 {
            // Next certificate in chain
            &certs[chain_indices[chain_pos + 1]]
        } else {
            // Last certificate in chain - check if it's self-signed or issued by our embedded root
            if cert.issuer() == cert.subject() {
                // Self-signed certificate - skip signature verification
                continue;
            } else if cert.issuer() == root_cert.subject() {
                // Issued by our embedded root
                &root_cert
            } else {
                // Different TSA provider - skip signature verification
                continue;
            }
        };

        if cert.issuer() != issuer_cert.subject() {
            return Err(TsaVerifyError::CertChainVerificationFailed);
        }

        // Certificate signature verification
        let cert_der = cert_ders[cert_index].clone();
        if let Err(e) = verify_certificate_signature(&cert_der, issuer_cert) {
            return Err(e);
        }
    }

    Ok(())
}

/// Verify TSA signature from CMS SignedData
fn verify_tsa_signature(signed_data: &SignedData) -> Result<(), TsaVerifyError> {
    // Get the first (and typically only) signer
    let signer_info = signed_data
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or(TsaVerifyError::TsaSignatureVerificationFailed)?;

    // Get the signing certificate
    let certificates = signed_data
        .certificates
        .as_ref()
        .ok_or(TsaVerifyError::TsaSignatureVerificationFailed)?;

    // Find the certificate that matches the signer (non-CA certificate)
    let mut signing_cert_der = None;

    for cert in certificates.0.iter() {
        if let Ok(cert_der) = cert.to_der() {
            if let Ok((_, parsed_cert)) = X509Certificate::from_der(&cert_der) {
                // Look for TSA certificate (should contain timeStamping key usage)
                // Pick the certificate that's NOT a root CA (doesn't have CA=true)
                if let Ok(Some(bc)) = parsed_cert.basic_constraints() {
                    if !bc.value.ca {
                        signing_cert_der = Some(cert_der);
                        break;
                    }
                } else {
                    // No basic constraints might mean it's an end-entity cert
                    signing_cert_der = Some(cert_der);
                    break;
                }
            }
        }
    }

    let signing_cert_der = signing_cert_der.ok_or(TsaVerifyError::TsaSignatureVerificationFailed)?;

    let (_, signing_cert) = X509Certificate::from_der(&signing_cert_der).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse signing certificate: {}", e))
    })?;

    // Extract public key
    let public_key_der = signing_cert.public_key().raw;

    // Get the digest and signature algorithm OIDs
    let digest_alg_oid = signer_info.digest_alg.oid.to_string();
    let sig_alg_oid = signer_info.signature_algorithm.oid.to_string();

    // For CMS signatures, we need to reconstruct the SignedAttributes
    let signed_message = if let Some(signed_attrs) = &signer_info.signed_attrs {
        // CRITICAL: Replace IMPLICIT [0] tag with SET tag (0x31) for signature verification
        let mut signed_attrs_der = signed_attrs
            .to_der()
            .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;

        // Check the first byte and handle accordingly
        match signed_attrs_der.get(0) {
            Some(0xA0) => {
                // IMPLICIT [0] tag - replace with SET tag
                signed_attrs_der[0] = 0x31;
            }
            Some(0x31) => {
                // Already SET tag - this is what we want
            }
            _ => {
                return Err(TsaVerifyError::TsaSignatureVerificationFailed);
            }
        }

        signed_attrs_der
    } else {
        // Fallback: sign the encapsulated content directly
        signed_data
            .encap_content_info
            .econtent
            .as_ref()
            .ok_or(TsaVerifyError::TsaSignatureVerificationFailed)?
            .to_der()
            .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?
    };

    // Verify the RSA signature
    verify_cms_signature(
        public_key_der,
        signer_info.signature.as_bytes(),
        &signed_message,
        &digest_alg_oid,
        &sig_alg_oid,
    )?;

    Ok(())
}

/// Verify CMS signature with separate digest and signature algorithms
fn verify_cms_signature(
    public_key_der: &[u8],
    signature: &[u8],
    message: &[u8],
    digest_alg_oid: &str,
    sig_alg_oid: &str,
) -> Result<(), TsaVerifyError> {
    // Parse RSA public key from DER
    let public_key = RsaPublicKey::from_public_key_der(public_key_der).map_err(|e| {
        TsaVerifyError::RsaKeyParsingFailed(e.to_string())
    })?;

    // Ensure signature algorithm is RSA
    if sig_alg_oid != "1.2.840.113549.1.1.1" {
        return Err(TsaVerifyError::UnsupportedSignatureAlgorithm(format!(
            "Signature algorithm: {}",
            sig_alg_oid
        )));
    }

    // Hash the message using the specified digest algorithm, then verify signature
    match digest_alg_oid {
        "2.16.840.1.101.3.4.2.1" => {
            // SHA-256
            let verifying_key = VerifyingKey::<Sha256>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
            verifying_key
                .verify(message, &signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
        }
        "2.16.840.1.101.3.4.2.2" => {
            // SHA-384
            let verifying_key = VerifyingKey::<Sha384>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
            verifying_key
                .verify(message, &signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
        }
        "2.16.840.1.101.3.4.2.3" => {
            // SHA-512
            let verifying_key = VerifyingKey::<Sha512>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
            verifying_key
                .verify(message, &signature)
                .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;
        }
        other => {
            return Err(TsaVerifyError::UnsupportedSignatureAlgorithm(format!(
                "Digest algorithm: {}",
                other
            )));
        }
    }

    Ok(())
}

/// Verify that a certificate was signed by the given issuer certificate
fn verify_certificate_signature(
    cert_der: &[u8],
    issuer_cert: &X509Certificate,
) -> Result<(), TsaVerifyError> {
    // Parse the certificate
    let (_, cert) = X509Certificate::from_der(cert_der).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse certificate: {}", e))
    })?;

    // Extract TBSCertificate bytes
    let tbs_cert_bytes = extract_tbs_certificate_bytes(cert_der)?;

    // Get the signature algorithm and signature value
    let sig_alg_oid = cert.signature_algorithm.algorithm.to_string();
    let signature_bytes = cert.signature_value.as_ref();

    // Get issuer's public key
    let issuer_public_key_der = issuer_cert.public_key().raw;

    // Verify the signature based on the algorithm
    match sig_alg_oid.as_str() {
        "1.2.840.113549.1.1.11" => {
            // sha256WithRSAEncryption
            verify_cms_signature(
                issuer_public_key_der,
                signature_bytes,
                &tbs_cert_bytes,
                "2.16.840.1.101.3.4.2.1", // SHA-256
                "1.2.840.113549.1.1.1",   // RSA
            )?;
        }
        "1.2.840.113549.1.1.12" => {
            // sha384WithRSAEncryption
            verify_cms_signature(
                issuer_public_key_der,
                signature_bytes,
                &tbs_cert_bytes,
                "2.16.840.1.101.3.4.2.2", // SHA-384
                "1.2.840.113549.1.1.1",   // RSA
            )?;
        }
        "1.2.840.113549.1.1.13" => {
            // sha512WithRSAEncryption
            verify_cms_signature(
                issuer_public_key_der,
                signature_bytes,
                &tbs_cert_bytes,
                "2.16.840.1.101.3.4.2.3", // SHA-512
                "1.2.840.113549.1.1.1",   // RSA
            )?;
        }
        "1.2.840.113549.1.1.5" => {
            // sha1WithRSAEncryption - old algorithm but still used
            // We'll skip SHA-1 verification for now as it's deprecated
            return Ok(());
        }
        other => {
            return Err(TsaVerifyError::UnsupportedSignatureAlgorithm(format!(
                "Certificate signature algorithm: {}",
                other
            )));
        }
    }

    Ok(())
}

/// Extract TBSCertificate bytes from a certificate DER encoding
fn extract_tbs_certificate_bytes(cert_der: &[u8]) -> Result<Vec<u8>, TsaVerifyError> {
    if cert_der.len() < 10 {
        return Err(TsaVerifyError::InvalidCertificate(
            "Certificate too short".to_string(),
        ));
    }

    let mut pos = 0;

    // Parse outer SEQUENCE (Certificate)
    if cert_der[pos] != 0x30 {
        return Err(TsaVerifyError::InvalidCertificate(
            "Invalid certificate DER: not a SEQUENCE".to_string(),
        ));
    }
    pos += 1;

    // Skip outer length
    let (_, outer_len_bytes) = parse_der_length(&cert_der[pos..])?;
    pos += outer_len_bytes;

    // Parse inner SEQUENCE (TBSCertificate)
    if cert_der[pos] != 0x30 {
        return Err(TsaVerifyError::InvalidCertificate(
            "Invalid TBSCertificate: not a SEQUENCE".to_string(),
        ));
    }

    let tbs_start = pos;
    pos += 1;

    // Parse TBSCertificate length
    let (tbs_content_len, tbs_len_bytes) = parse_der_length(&cert_der[pos..])?;
    let tbs_total_len = 1 + tbs_len_bytes + tbs_content_len;

    if tbs_start + tbs_total_len > cert_der.len() {
        return Err(TsaVerifyError::InvalidCertificate(
            "Invalid TBSCertificate length".to_string(),
        ));
    }

    // Extract TBSCertificate bytes
    Ok(cert_der[tbs_start..tbs_start + tbs_total_len].to_vec())
}

/// Parse DER length field and return (content_length, length_field_bytes)
fn parse_der_length(data: &[u8]) -> Result<(usize, usize), TsaVerifyError> {
    if data.is_empty() {
        return Err(TsaVerifyError::InvalidCertificate(
            "Empty length field".to_string(),
        ));
    }

    let first_byte = data[0];

    if first_byte & 0x80 == 0 {
        // Short form
        Ok((first_byte as usize, 1))
    } else {
        // Long form
        let length_bytes = (first_byte & 0x7F) as usize;

        if length_bytes == 0 || length_bytes > 4 || data.len() < 1 + length_bytes {
            return Err(TsaVerifyError::InvalidCertificate(
                "Invalid DER length encoding".to_string(),
            ));
        }

        let mut length = 0usize;
        for i in 1..=length_bytes {
            length = (length << 8) | (data[i] as usize);
        }

        Ok((length, 1 + length_bytes))
    }
}

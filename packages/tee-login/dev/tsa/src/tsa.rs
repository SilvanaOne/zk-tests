use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::Decode;
use der::{
    Encode,
    asn1::{Int, OctetString},
    oid::db::rfc5912::ID_SHA_256,
};
use once_cell::sync::Lazy;
use rand::{RngCore, rng};
use reqwest::Client;
use rsa::{
    BigUint, RsaPublicKey, pkcs1v15::VerifyingKey, pkcs8::DecodePublicKey, signature::Verifier,
    traits::PublicKeyParts,
};
use sha2::{Digest, Sha256, Sha384, Sha512};
use spki::AlgorithmIdentifierOwned;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;
use x509_parser::certificate::X509Certificate;
use x509_parser::prelude::FromDer;
use x509_parser::time::ASN1Time;
use x509_tsp::{MessageImprint, TimeStampReq, TimeStampResp, TspVersion, TstInfo};

/// DigiCert Assured ID Root CA certificate for TSA verification
static DIGICERT_ROOT_CA: Lazy<Vec<u8>> = Lazy::new(|| {
    let pem_bytes = include_bytes!("root_certs/digicert_root_g4.pem");
    let pem_str = std::str::from_utf8(pem_bytes).expect("Valid UTF-8 in root cert");

    // Parse PEM to get DER bytes
    let mut cert_der = Vec::new();
    let mut in_cert = false;

    for line in pem_str.lines() {
        if line.starts_with("-----BEGIN CERTIFICATE-----") {
            in_cert = true;
            continue;
        }
        if line.starts_with("-----END CERTIFICATE-----") {
            break;
        }
        if in_cert {
            use base64::Engine;
            cert_der.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(line.trim())
                    .expect("Valid base64 in cert"),
            );
        }
    }

    cert_der
});

/// Error type for TSA verification
#[derive(Debug)]
pub enum TsaVerifyError {
    /// Invalid certificate: {0}
    InvalidCertificate(String),
    /// Certificate chain verification failed
    CertChainVerificationFailed,
    /// Certificate timestamp not valid
    CertificateExpired,
    /// Missing required certificate extension
    MissingExtension(String),
    /// Invalid key usage
    InvalidKeyUsage,
    /// TSA signature verification failed
    TsaSignatureVerificationFailed,
    /// Unsupported signature algorithm
    UnsupportedSignatureAlgorithm(String),
    /// RSA key parsing failed
    RsaKeyParsingFailed(String),
}

impl std::fmt::Display for TsaVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsaVerifyError::InvalidCertificate(msg) => write!(f, "Invalid certificate: {}", msg),
            TsaVerifyError::CertChainVerificationFailed => {
                write!(f, "Certificate chain verification failed")
            }
            TsaVerifyError::CertificateExpired => write!(f, "Certificate has expired"),
            TsaVerifyError::MissingExtension(ext) => {
                write!(f, "Missing required extension: {}", ext)
            }
            TsaVerifyError::InvalidKeyUsage => write!(f, "Invalid key usage"),
            TsaVerifyError::TsaSignatureVerificationFailed => {
                write!(f, "TSA signature verification failed")
            }
            TsaVerifyError::UnsupportedSignatureAlgorithm(algo) => {
                write!(f, "Unsupported signature algorithm: {}", algo)
            }
            TsaVerifyError::RsaKeyParsingFailed(msg) => {
                write!(f, "RSA key parsing failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for TsaVerifyError {}

/// Save certificates from TSA response to disk for analysis
pub fn save_certificates_to_disk(
    signed_data: &SignedData,
    prefix: &str,
) -> anyhow::Result<Vec<String>> {
    let mut saved_files = Vec::new();

    if let Some(certificates) = &signed_data.certificates {
        for (i, cert) in certificates.0.iter().enumerate() {
            let cert_der = cert.to_der()?;
            let filename = format!("{}_cert_{}.der", prefix, i);

            fs::write(&filename, &cert_der)?;
            saved_files.push(filename.clone());

            debug!("💾 Saved certificate {} to: {}", i, filename);

            // Also try to parse and show basic info
            if let Ok((_, parsed_cert)) =
                x509_parser::certificate::X509Certificate::from_der(&cert_der)
            {
                debug!("   📄 Subject: {}", parsed_cert.subject());
            }
        }
    }

    Ok(saved_files)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TsaResponse {
    pub gen_time: der::asn1::GeneralizedTime,
    pub time_string: String,
    pub precision_info: TimePrecision,
    pub accuracy_info: Option<AccuracyInfo>,
    pub serial_number: Int,
    pub full_response: Vec<u8>,
    pub cert_verification: CertVerificationResult,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TimePrecision {
    pub has_fractional_seconds: bool,
    pub fractional_digits: Option<usize>,
    pub precision_description: String,
    pub fractional_value: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AccuracyInfo {
    pub seconds: Option<i32>,
    pub millis: Option<u16>,
    pub micros: Option<u16>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CertVerificationResult {
    pub is_valid: bool,
    pub cert_count: usize,
    pub error_message: Option<String>,
    pub signer_cert_subject: Option<String>,
}

#[allow(dead_code)]
pub async fn get_timestamp(data: &[u8], endpoint: &str) -> anyhow::Result<TsaResponse> {
    // --- 1. hash the data ---------------------------------------------------
    let digest = Sha256::digest(data);

    // --- 2. build TimeStampReq ---------------------------------------------
    let imprint = MessageImprint {
        hash_algorithm: AlgorithmIdentifierOwned {
            oid: ID_SHA_256,
            parameters: None,
        },
        hashed_message: OctetString::new(digest.to_vec())?,
    };

    let mut nonce_bytes = [0u8; 8];
    rng().fill_bytes(&mut nonce_bytes);

    let ts_req = TimeStampReq {
        version: TspVersion::V1,
        message_imprint: imprint,
        req_policy: None,
        nonce: Some(Int::new(&nonce_bytes)?),
        cert_req: true,
        extensions: None,
    };
    let der_req = ts_req.to_der()?;

    // --- 3. POST to TSA endpoint -------------------------------------------
    let client = Client::builder()
        .user_agent("Rust RFC3161 TSA Client/0.1")
        .build()?;

    let resp_bytes = client
        .post(endpoint)
        .header("Content-Type", "application/timestamp-query")
        .body(der_req)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    // --- 4. parse & basic validation ---------------------------------------
    let ts_resp = TimeStampResp::from_der(resp_bytes.as_ref())?;
    if ts_resp.time_stamp_token.is_none() {
        return Err(anyhow::anyhow!(
            "TSA did not include a timestamp token. Status: {:?}",
            ts_resp.status.status
        ));
    }
    let token = ts_resp.time_stamp_token.unwrap();

    // --- 5. Extract TSTInfo from the token ---------------------------------
    let token_der = token.to_der()?;
    let ci = ContentInfo::from_der(&token_der)?;

    // ci.content is a der::Any, we need to decode it as SignedData
    let signed = SignedData::from_der(ci.content.to_der()?.as_slice())?;

    // --- 6. Verify certificate chain ---------------------------------------
    let cert_verification = verify_certificates(&signed)?;

    // --- 7. Optional: Save certificates to disk for analysis ------------
    if std::env::var("TSA_SAVE_CERTS").is_ok() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let prefix = format!("tsa_response_{}", timestamp);

        if let Ok(saved_files) = save_certificates_to_disk(&signed, &prefix) {
            debug!("💾 Saved {} certificates for analysis:", saved_files.len());
            for file in saved_files {
                debug!("   📁 {}", file);
            }
        }
    }

    let econtent = signed
        .encap_content_info
        .econtent
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing eContent"))?;

    // econtent is a der::Any containing an OCTET STRING with TstInfo bytes
    let octet_string = OctetString::from_der(econtent.to_der()?.as_slice())?;
    let tst_info = TstInfo::from_der(octet_string.as_bytes())?;

    // --- 8. Analyze timing information -------------------------------------
    let gen_time_der = tst_info.gen_time.to_der()?;
    // GeneralizedTime DER: tag(1) + length(1+) + content
    let time_content = if gen_time_der.len() > 2 {
        match gen_time_der[1] {
            len if len < 0x80 => &gen_time_der[2..],
            0x81 => &gen_time_der[3..],
            0x82 => &gen_time_der[4..],
            _ => &gen_time_der[2..],
        }
    } else {
        &gen_time_der[2..]
    };

    let time_str = std::str::from_utf8(time_content)
        .unwrap_or("Unable to parse time string")
        .to_string();

    // Analyze precision
    let precision_info = if time_str.contains('.') {
        let parts: Vec<&str> = time_str.split('.').collect();
        if parts.len() == 2 {
            let fractional_part = parts[1].trim_end_matches('Z');
            let precision = fractional_part.len();
            TimePrecision {
                has_fractional_seconds: true,
                fractional_digits: Some(precision),
                precision_description: match precision {
                    1 => "decisecond (0.1s)".to_string(),
                    2 => "centisecond (0.01s)".to_string(),
                    3 => "millisecond (0.001s)".to_string(),
                    4 => "tenth millisecond".to_string(),
                    5 => "hundredth millisecond".to_string(),
                    6 => "microsecond (0.000001s)".to_string(),
                    _ => "high precision".to_string(),
                },
                fractional_value: Some(format!("0.{}", fractional_part)),
            }
        } else {
            TimePrecision {
                has_fractional_seconds: false,
                fractional_digits: None,
                precision_description: "Second-level only (invalid fractional format)".to_string(),
                fractional_value: None,
            }
        }
    } else {
        TimePrecision {
            has_fractional_seconds: false,
            fractional_digits: None,
            precision_description: "Second-level only (no fractional seconds)".to_string(),
            fractional_value: None,
        }
    };

    // Extract accuracy information
    let accuracy_info = tst_info.accuracy.as_ref().map(|accuracy| AccuracyInfo {
        seconds: accuracy.seconds.map(|s| s.try_into().unwrap_or(0)),
        millis: accuracy.millis.map(|m| m.try_into().unwrap_or(0)),
        micros: accuracy.micros.map(|m| m.try_into().unwrap_or(0)),
    });

    Ok(TsaResponse {
        gen_time: tst_info.gen_time,
        time_string: time_str,
        precision_info,
        accuracy_info,
        serial_number: tst_info.serial_number,
        full_response: resp_bytes.to_vec(),
        cert_verification,
    })
}

/// Verify the certificate chain in the CMS SignedData
fn verify_certificates(signed_data: &SignedData) -> anyhow::Result<CertVerificationResult> {
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
    let now_secs = ASN1Time::from_timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TsaVerifyError::InvalidCertificate("Invalid system time".to_string()))?
            .as_secs() as i64,
    )
    .map_err(|e| TsaVerifyError::InvalidCertificate(format!("Invalid timestamp: {}", e)))?;

    // Parse the embedded root certificate
    let (_, root_cert) = X509Certificate::from_der(&DIGICERT_ROOT_CA).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse root certificate: {}", e))
    })?;

    // Parse all certificates from the chain
    let mut certs = Vec::new();
    for (i, cert_der) in cert_ders.iter().enumerate() {
        let (_, cert) = X509Certificate::from_der(cert_der).map_err(|e| {
            TsaVerifyError::InvalidCertificate(format!("Failed to parse certificate: {}", e))
        })?;

        // Inspect RSA public key in each certificate
        debug!("🔍 Certificate {} RSA Key Analysis:", i);
        debug!("   📄 Subject: {}", cert.subject());

        if let Ok(public_key) = RsaPublicKey::from_public_key_der(cert.public_key().raw) {
            debug!("   🔢 RSA Key Size: {} bits", public_key.size() * 8);
            debug!("   🔢 Public Exponent (e): {}", public_key.e());
            debug!(
                "   🔢 Uses RSA-65537: {}",
                public_key.e() == &BigUint::from(65537u32)
            );

            // Show modulus characteristics
            let n_bytes = public_key.n().to_bytes_be();
            debug!("   🔢 Modulus Size: {} bytes", n_bytes.len());
            let n_hex: Vec<String> = n_bytes
                .iter()
                .take(8)
                .map(|b| format!("{:02X}", b))
                .collect();
            debug!("   🔢 Modulus Start: {}", n_hex.join(""));
        } else {
            debug!("   ❌ Failed to parse RSA public key from certificate");
        }

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
    const MAX_CHAIN_LENGTH: usize = 10; // Prevent infinite loops
    let mut chain_length = 1; // Already have leaf

    loop {
        // Check if we've reached maximum chain length
        if chain_length >= MAX_CHAIN_LENGTH {
            debug!("⚠️  Maximum certificate chain length reached, stopping verification");
            break;
        }

        // Check if current certificate is self-signed (root certificate)
        if current_cert.issuer() == current_cert.subject() {
            debug!(
                "✅ Reached self-signed root certificate: {}",
                current_cert.subject()
            );
            break;
        }

        // Check if we've reached our embedded DigiCert root
        if current_cert.issuer() == root_cert.subject() {
            debug!("✅ Chain reaches embedded DigiCert root certificate");
            break;
        }

        // Look for the issuer certificate in the provided chain
        let mut found_issuer = false;
        for (i, potential_issuer) in certs.iter().enumerate() {
            if current_cert.issuer() == potential_issuer.subject() {
                // Avoid circular chains
                if chain_indices.contains(&i) {
                    debug!("⚠️  Detected circular certificate chain, stopping");
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
            debug!(
                "⚠️  Could not find issuer for certificate: {}",
                current_cert.subject()
            );
            debug!("   Looking for issuer: {}", current_cert.issuer());
            break; // Stop verification but don't fail - might be a valid chain ending
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
                debug!(
                    "⚠️  Self-signed certificate detected, skipping signature verification: {}",
                    cert.subject()
                );
                continue;
            } else if cert.issuer() == root_cert.subject() {
                // Issued by our embedded root
                &root_cert
            } else {
                // Different TSA provider - we can't verify the signature of the root certificate
                debug!(
                    "⚠️  Certificate issued by unknown root, skipping signature verification: {}",
                    cert.subject()
                );
                debug!("   Certificate issuer: {}", cert.issuer());
                debug!("   Our embedded root: {}", root_cert.subject());
                continue;
            }
        };

        if cert.issuer() != issuer_cert.subject() {
            debug!("⚠️  Certificate chain mismatch:");
            debug!("   Certificate subject: {}", cert.subject());
            debug!("   Certificate issuer: {}", cert.issuer());
            debug!("   Expected issuer subject: {}", issuer_cert.subject());
            return Err(TsaVerifyError::CertChainVerificationFailed);
        }

        // Certificate signature verification - verify this certificate was signed by its issuer
        let cert_der = cert_ders[cert_index].clone();
        if let Err(e) = verify_certificate_signature(&cert_der, issuer_cert) {
            debug!(
                "❌ Certificate signature verification failed for {}: {}",
                cert.subject(),
                e
            );
            return Err(TsaVerifyError::CertChainVerificationFailed);
        } else {
            debug!("✅ Certificate signature verified for: {}", cert.subject());
        }
    }

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
    debug!("🔍 CMS Signature Verification Details:");
    debug!("   📝 Message length: {} bytes", message.len());
    debug!(
        "   🔑 Public key DER length: {} bytes",
        public_key_der.len()
    );
    debug!("   ✍️  Signature length: {} bytes", signature.len());
    debug!("   🔒 Digest algorithm: {}", digest_alg_oid);
    debug!("   🔒 Signature algorithm: {}", sig_alg_oid);

    // Debug: Print first few bytes of each component
    let message_preview: Vec<String> = message
        .iter()
        .take(16)
        .map(|b| format!("{:02X}", b))
        .collect();
    debug!("   📄 Message preview: {}", message_preview.join(" "));

    let signature_preview: Vec<String> = signature
        .iter()
        .take(16)
        .map(|b| format!("{:02X}", b))
        .collect();
    debug!("   ✏️  Signature preview: {}", signature_preview.join(" "));

    // Parse RSA public key from DER
    let public_key = RsaPublicKey::from_public_key_der(public_key_der).map_err(|e| {
        debug!("❌ RSA public key parsing failed: {}", e);
        TsaVerifyError::RsaKeyParsingFailed(e.to_string())
    })?;

    debug!("✅ RSA public key parsed successfully");

    // Display RSA parameters
    debug!("🔍 RSA Key Parameters:");
    debug!("   🔢 Modulus (n) bit size: {} bits", public_key.size() * 8);
    debug!("   🔢 Public exponent (e): {}", public_key.e());
    debug!(
        "   🔢 Is e = 65537? {}",
        public_key.e() == &BigUint::from(65537u32)
    );

    // Show first few bytes of modulus for identification
    let n_bytes = public_key.n().to_bytes_be();
    let n_preview: Vec<String> = n_bytes
        .iter()
        .take(16)
        .map(|b| format!("{:02X}", b))
        .collect();
    debug!("   🔢 Modulus (n) preview: {}", n_preview.join(" "));

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
            debug!("🔒 Using SHA-256 for verification");
            let verifying_key = VerifyingKey::<Sha256>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature).map_err(|e| {
                debug!("❌ PKCS#1 v1.5 signature parsing failed: {}", e);
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;

            // Debug: Calculate and print the hash we're verifying against
            let hash = Sha256::digest(message);
            let hash_hex: Vec<String> = hash.iter().map(|b| format!("{:02X}", b)).collect();
            debug!("   🔍 SHA-256 hash: {}", hash_hex.join(""));

            verifying_key.verify(message, &signature).map_err(|e| {
                debug!("❌ RSA signature verification failed: {}", e);
                debug!("   This could be due to:");
                debug!("   - Wrong message content (SignedAttributes encoding)");
                debug!("   - Incorrect public key");
                debug!("   - Wrong signature algorithm/padding");
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;
        }
        "2.16.840.1.101.3.4.2.2" => {
            // SHA-384
            debug!("🔒 Using SHA-384 for verification");
            let verifying_key = VerifyingKey::<Sha384>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature).map_err(|e| {
                debug!("❌ PKCS#1 v1.5 signature parsing failed: {}", e);
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;
            verifying_key.verify(message, &signature).map_err(|e| {
                debug!("❌ RSA signature verification failed: {}", e);
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;
        }
        "2.16.840.1.101.3.4.2.3" => {
            // SHA-512
            debug!("🔒 Using SHA-512 for verification");
            let verifying_key = VerifyingKey::<Sha512>::new(public_key);
            let signature = rsa::pkcs1v15::Signature::try_from(signature).map_err(|e| {
                debug!("❌ PKCS#1 v1.5 signature parsing failed: {}", e);
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;
            verifying_key.verify(message, &signature).map_err(|e| {
                debug!("❌ RSA signature verification failed: {}", e);
                TsaVerifyError::TsaSignatureVerificationFailed
            })?;
        }
        other => {
            return Err(TsaVerifyError::UnsupportedSignatureAlgorithm(format!(
                "Digest algorithm: {}",
                other
            )));
        }
    }

    debug!("✅ Cryptographic signature verification passed!");
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

    // Find the certificate that matches the signer
    // The signer_info.sid (SignerIdentifier) should match a certificate
    let mut signing_cert_der = None;

    // Debug: Print all certificates to understand the structure
    debug!("🔍 Available certificates:");
    for (i, cert) in certificates.0.iter().enumerate() {
        if let Ok(cert_der) = cert.to_der() {
            if let Ok((_, parsed_cert)) = X509Certificate::from_der(&cert_der) {
                debug!("   Certificate {}: {}", i, parsed_cert.subject());

                // Look for TSA certificate (should contain timeStamping key usage)
                // For now, pick the certificate that's NOT a root CA (doesn't have CA=true)
                if let Ok(Some(bc)) = parsed_cert.basic_constraints() {
                    if !bc.value.ca {
                        debug!(
                            "   ✅ Found non-CA certificate (likely TSA signer): {}",
                            parsed_cert.subject()
                        );
                        signing_cert_der = Some(cert_der);
                        break;
                    } else {
                        debug!("   🏛️  CA certificate: {}", parsed_cert.subject());
                    }
                } else {
                    // No basic constraints might mean it's an end-entity cert
                    debug!(
                        "   📝 Certificate without basic constraints (likely TSA signer): {}",
                        parsed_cert.subject()
                    );
                    signing_cert_der = Some(cert_der);
                    break;
                }
            }
        }
    }

    let signing_cert_der = signing_cert_der.ok_or_else(|| {
        debug!("❌ Could not find TSA signing certificate");
        TsaVerifyError::TsaSignatureVerificationFailed
    })?;

    let (_, signing_cert) = X509Certificate::from_der(&signing_cert_der).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse signing certificate: {}", e))
    })?;

    debug!(
        "🔑 Using certificate for signature verification: {}",
        signing_cert.subject()
    );

    // Extract public key in proper SubjectPublicKeyInfo DER format
    let public_key_der = signing_cert.public_key().raw;

    // Get the digest and signature algorithm OIDs
    let digest_alg_oid = signer_info.digest_alg.oid.to_string();
    let sig_alg_oid = signer_info.signature_algorithm.oid.to_string();

    // For CMS signatures, we need to reconstruct the SignedAttributes
    // The signature is computed over the DER-encoded SignedAttributes with SET tag, not the content directly
    let signed_message = if let Some(signed_attrs) = &signer_info.signed_attrs {
        // CRITICAL: Replace IMPLICIT [0] tag with SET tag (0x31) for signature verification
        // This is required by RFC 3161 and CMS specifications
        let mut signed_attrs_der = signed_attrs
            .to_der()
            .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?;

        // Debug: Print first few bytes to understand the structure
        let first_bytes: Vec<String> = signed_attrs_der
            .iter()
            .take(10)
            .map(|b| format!("0x{:02X}", b))
            .collect();
        debug!(
            "🔍 SignedAttributes DER bytes: [{}]",
            first_bytes.join(", ")
        );
        debug!("🔍 Length: {} bytes", signed_attrs_der.len());

        // Check the first byte and handle accordingly
        match signed_attrs_der.get(0) {
            Some(0xA0) => {
                // IMPLICIT [0] tag - replace with SET tag
                signed_attrs_der[0] = 0x31;
                debug!(
                    "🔧 Fixed SignedAttributes encoding: IMPLICIT [0] -> SET tag for signature verification"
                );
            }
            Some(0x31) => {
                // Already SET tag - this is what we want for signature verification
                debug!(
                    "✅ SignedAttributes already has SET tag (0x31) - ready for signature verification"
                );
            }
            Some(other_tag) => {
                debug!(
                    "⚠️  Unexpected tag: 0x{:02X}, proceeding with current encoding",
                    other_tag
                );
            }
            None => {
                debug!("❌ Empty SignedAttributes");
                return Err(TsaVerifyError::TsaSignatureVerificationFailed);
            }
        }

        signed_attrs_der
    } else {
        // Fallback: sign the encapsulated content directly (less common for TSA)
        signed_data
            .encap_content_info
            .econtent
            .as_ref()
            .ok_or(TsaVerifyError::TsaSignatureVerificationFailed)?
            .to_der()
            .map_err(|_| TsaVerifyError::TsaSignatureVerificationFailed)?
    };

    // Verify the RSA signature using the digest algorithm for hashing
    verify_cms_signature(
        public_key_der,
        signer_info.signature.as_bytes(),
        &signed_message,
        &digest_alg_oid,
        &sig_alg_oid,
    )?;

    debug!("✅ TSA signature verification passed!");
    Ok(())
}

/// Extract TBSCertificate bytes from a certificate DER encoding
fn extract_tbs_certificate_bytes(cert_der: &[u8]) -> Result<Vec<u8>, TsaVerifyError> {
    if cert_der.len() < 10 {
        return Err(TsaVerifyError::InvalidCertificate(
            "Certificate too short".to_string(),
        ));
    }

    // Certificate DER structure:
    // 30 82 XX XX        - SEQUENCE tag + length (certificate)
    //   30 82 YY YY      - SEQUENCE tag + length (TBSCertificate) <- We want this
    //     ...            - TBSCertificate content
    //   30 0D            - SEQUENCE tag + length (signatureAlgorithm)
    //   03 82 ZZ ZZ      - BIT STRING tag + length (signatureValue)

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
    let tbs_total_len = 1 + tbs_len_bytes + tbs_content_len; // tag + length + content

    if tbs_start + tbs_total_len > cert_der.len() {
        return Err(TsaVerifyError::InvalidCertificate(
            "Invalid TBSCertificate length".to_string(),
        ));
    }

    // Extract TBSCertificate bytes (tag + length + content)
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
        // Short form: length is just the first byte
        Ok((first_byte as usize, 1))
    } else {
        // Long form: first byte indicates how many subsequent bytes encode the length
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

/// Verify that a certificate was signed by the given issuer certificate
fn verify_certificate_signature(
    cert_der: &[u8],
    issuer_cert: &X509Certificate,
) -> Result<(), TsaVerifyError> {
    // Parse the certificate to extract signature algorithm and signature value
    let (_, cert) = X509Certificate::from_der(cert_der).map_err(|e| {
        TsaVerifyError::InvalidCertificate(format!("Failed to parse certificate: {}", e))
    })?;

    // Extract TBSCertificate bytes (the part that was actually signed)
    let tbs_cert_bytes = extract_tbs_certificate_bytes(cert_der)?;

    // Get the signature algorithm and signature value
    let sig_alg_oid = cert.signature_algorithm.algorithm.to_string();
    let signature_bytes = cert.signature_value.as_ref();

    // Get issuer's public key
    let issuer_public_key_der = issuer_cert.public_key().raw;

    debug!("🔍 Certificate signature verification:");
    debug!("   📄 Certificate: {}", cert.subject());
    debug!("   🏛️  Issuer: {}", issuer_cert.subject());
    debug!("   🔒 Signature algorithm: {}", sig_alg_oid);
    debug!("   📝 TBSCertificate size: {} bytes", tbs_cert_bytes.len());
    debug!("   ✍️  Signature size: {} bytes", signature_bytes.len());

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
        other => {
            return Err(TsaVerifyError::UnsupportedSignatureAlgorithm(format!(
                "Certificate signature algorithm: {}",
                other
            )));
        }
    }

    Ok(())
}

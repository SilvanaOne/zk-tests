use crate::db::Share;
use crate::logger::log_encryption_error;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use cms::{content_info::ContentInfo, enveloped_data::EnvelopedData};
use der::Decode;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey, pkcs8::DecodePublicKey, rand_core::OsRng};
use sha2::{Sha256, Sha512};
use zeroize::Zeroizing;

#[derive(Debug, Clone)]
pub struct KMSKeyPair {
    pub public_key: RsaPublicKey,
    pub private_key: RsaPrivateKey,
}

pub fn generate_kms_key_pair() -> Result<KMSKeyPair> {
    let mut rng = OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
    let public_key = RsaPublicKey::from(&private_key);
    Ok(KMSKeyPair {
        public_key,
        private_key,
    })
}

pub fn decrypt(data: &[u8], private_key: &RsaPrivateKey) -> Result<Vec<u8>> {
    let padding = Oaep::new::<Sha256>();
    let plaintext = private_key
        .decrypt(padding, data)
        .context("KMS decryption failed")?;
    Ok(plaintext)
}

pub fn encrypt(data: &[u8], public_key: &str) -> Result<String> {
    let der = B64.decode(public_key).context("bad base64")?;
    let rsa_pub = RsaPublicKey::from_public_key_der(&der).context("not an RSA public key")?;

    let mut rng = OsRng;
    let padding = Oaep::new::<Sha512>();
    let cipher = rsa_pub
        .encrypt(&mut rng, padding, data)
        .context("encryption failed")?;

    Ok(B64.encode(cipher))
}

pub fn encrypt_shares(shares: &[Share], public_key: &str) -> Result<Vec<String>> {
    let mut encrypted_shares = Vec::new();
    for share in shares {
        let plaintext = Zeroizing::new(match bincode::serialize(&share) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Failed to serialize shares: {}", e);
                log_encryption_error(&error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }
        });

        let restored_data = match bincode::deserialize::<Share>(&plaintext) {
            Ok(data) => data,
            Err(e) => {
                let error_msg = format!("Failed to deserialize shares: {}", e);
                log_encryption_error(&error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }
        };
        assert_eq!(restored_data.index, share.index);
        assert_eq!(restored_data.data, share.data);

        encrypted_shares.push(encrypt(&plaintext, public_key)?);
    }
    Ok(encrypted_shares)
}

/// Decrypt PKCS#7 EnvelopedData from AWS KMS CiphertextForRecipient
/// This is specifically for decrypting the symmetric key from AWS KMS when using Nitro Enclaves
pub fn decrypt_kms_ciphertext(data: &[u8], private_key: &RsaPrivateKey) -> Result<Vec<u8>> {
    // AWS KMS returns the symmetric key encrypted in PKCS#7 EnvelopedData format
    // We need to parse this structure and extract the RSA-encrypted synthetic key

    // Debug: log first few bytes to understand the structure
    let first_bytes: Vec<String> = data.iter().take(20).map(|b| format!("{:02x}", b)).collect();
    println!("KMS ciphertext first 20 bytes: {}", first_bytes.join(" "));
    println!("KMS ciphertext total length: {}", data.len());

    // Try parsing directly as EnvelopedData first (most common case)
    match EnvelopedData::from_der(data) {
        Ok(enveloped_data) => {
            println!("✓ Successfully parsed as EnvelopedData directly");
        }
        Err(e) => {
            println!("✗ Failed to parse as EnvelopedData directly: {}", e);
        }
    }

    if let Ok(enveloped_data) = EnvelopedData::from_der(data) {
        // Get the first recipient info (AWS KMS typically only has one)
        if let Some(recipient_info) = enveloped_data.recip_infos.0.get(0) {
            if let cms::enveloped_data::RecipientInfo::Ktri(ktri) = recipient_info {
                // Decrypt the symmetric key using RSA-OAEP with SHA-256
                let padding = Oaep::new::<Sha256>();
                match private_key.decrypt(padding, ktri.enc_key.as_bytes()) {
                    Ok(symmetric_key) => return Ok(symmetric_key),
                    Err(e) => {
                        log_encryption_error(&format!("RSA decryption failed: {}", e));
                    }
                }
            }
        }
        return Err(anyhow::anyhow!(
            "No valid recipient info found in EnvelopedData"
        ));
    }

    // Fallback: try parsing as ContentInfo wrapper
    match ContentInfo::from_der(data) {
        Ok(content_info) => {
            println!("✓ Successfully parsed as ContentInfo");
            // Extract the content and try to parse as EnvelopedData
            match EnvelopedData::from_der(content_info.content.value()) {
                Ok(enveloped_data) => {
                    println!("✓ Successfully extracted EnvelopedData from ContentInfo");
                }
                Err(e) => {
                    println!("✗ Failed to parse EnvelopedData from ContentInfo: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to parse as ContentInfo: {}", e);
        }
    }

    if let Ok(content_info) = ContentInfo::from_der(data) {
        // Extract the content and try to parse as EnvelopedData
        if let Ok(enveloped_data) = EnvelopedData::from_der(content_info.content.value()) {
            if let Some(recipient_info) = enveloped_data.recip_infos.0.get(0) {
                if let cms::enveloped_data::RecipientInfo::Ktri(ktri) = recipient_info {
                    let padding = Oaep::new::<Sha256>();
                    match private_key.decrypt(padding, ktri.enc_key.as_bytes()) {
                        Ok(symmetric_key) => return Ok(symmetric_key),
                        Err(e) => {
                            log_encryption_error(&format!("RSA decryption failed: {}", e));
                        }
                    }
                }
            }
        }
    }

    // Final fallback: try a more manual approach for indefinite length encoding
    println!("Trying manual parsing approach for indefinite length encoding...");

    // The data starts with SEQUENCE (0x30) with indefinite length (0x80)
    // This is a known issue with some PKCS#7 parsers
    if data.len() > 10 && data[0] == 0x30 && data[1] == 0x80 {
        // Convert indefinite length to definite length by finding the end-of-contents octets
        let mut definite_length_data = Vec::new();
        let mut i = 0;
        let mut found_eoc = false;

        while i < data.len() - 1 {
            if data[i] == 0x00 && data[i + 1] == 0x00 {
                // Found end-of-contents octets, calculate the actual length
                let content_length = i - 2; // Subtract 2 for the SEQUENCE tag and length byte
                definite_length_data.push(0x30); // SEQUENCE tag

                // Encode the definite length
                if content_length < 0x80 {
                    definite_length_data.push(content_length as u8);
                } else if content_length < 0x100 {
                    definite_length_data.push(0x81);
                    definite_length_data.push(content_length as u8);
                } else if content_length < 0x10000 {
                    definite_length_data.push(0x82);
                    definite_length_data.push((content_length >> 8) as u8);
                    definite_length_data.push((content_length & 0xFF) as u8);
                } else {
                    definite_length_data.push(0x83);
                    definite_length_data.push((content_length >> 16) as u8);
                    definite_length_data.push((content_length >> 8) as u8);
                    definite_length_data.push((content_length & 0xFF) as u8);
                }

                // Add the content (skip the original indefinite length byte)
                definite_length_data.extend_from_slice(&data[2..i]);
                found_eoc = true;
                break;
            }
            i += 1;
        }

        if found_eoc {
            println!("Converted indefinite length to definite length, trying to parse again...");

            // Try parsing the converted data as ContentInfo
            if let Ok(content_info) = ContentInfo::from_der(&definite_length_data) {
                println!("✓ Successfully parsed converted data as ContentInfo");
                if let Ok(enveloped_data) = EnvelopedData::from_der(content_info.content.value()) {
                    println!("✓ Successfully extracted EnvelopedData from converted ContentInfo");
                    if let Some(recipient_info) = enveloped_data.recip_infos.0.get(0) {
                        if let cms::enveloped_data::RecipientInfo::Ktri(ktri) = recipient_info {
                            let padding = Oaep::new::<Sha256>();
                            match private_key.decrypt(padding, ktri.enc_key.as_bytes()) {
                                Ok(symmetric_key) => {
                                    println!(
                                        "✓ Successfully decrypted symmetric key using converted data"
                                    );
                                    return Ok(symmetric_key);
                                }
                                Err(e) => {
                                    log_encryption_error(&format!(
                                        "RSA decryption failed on converted data: {}",
                                        e
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to parse AWS KMS ciphertext as PKCS#7 EnvelopedData or ContentInfo (tried indefinite length conversion)"
    ))
}

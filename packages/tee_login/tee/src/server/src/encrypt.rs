use crate::db::Share;
use crate::logger::log_encryption_error;
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
// CMS imports removed since we're using manual parsing for AWS KMS indefinite length encoding
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
    // However, it uses indefinite length encoding which the cms crate doesn't support
    // We'll manually extract the RSA-encrypted key from the known structure

    // Debug: log first few bytes to understand the structure
    let first_bytes: Vec<String> = data.iter().take(20).map(|b| format!("{:02x}", b)).collect();
    println!("KMS ciphertext first 20 bytes: {}", first_bytes.join(" "));
    println!("KMS ciphertext total length: {}", data.len());

    // AWS KMS PKCS#7 structure (with indefinite length):
    // 30 80 - ContentInfo SEQUENCE (indefinite)
    // 06 09 2a 86 48 86 f7 0d 01 07 03 - envelopedData OID
    // a0 80 - context specific 0 (indefinite) - content
    // 30 80 - EnvelopedData SEQUENCE (indefinite)
    // 02 01 02 - version
    // 31 ... - recipientInfos SET
    // 30 ... - KeyTransRecipientInfo SEQUENCE
    // ... 04 82 01 00 - OCTET STRING with the RSA-encrypted key (256 bytes for 2048-bit RSA)

    // Look for the pattern: 04 82 01 00 (OCTET STRING, definite length, 256 bytes)
    // This is where the RSA-encrypted symmetric key is stored
    for i in 0..data.len().saturating_sub(4) {
        if data[i] == 0x04 && data[i + 1] == 0x82 && data[i + 2] == 0x01 && data[i + 3] == 0x00 {
            // Found the OCTET STRING with 256 bytes (0x0100)
            let key_start = i + 4;
            let key_end = key_start + 256;

            if key_end <= data.len() {
                let encrypted_key = &data[key_start..key_end];
                println!(
                    "Found RSA-encrypted key at position {}, length: {}",
                    key_start,
                    encrypted_key.len()
                );

                // Try to decrypt the symmetric key using RSA-OAEP with SHA-256
                let padding = Oaep::new::<Sha256>();
                match private_key.decrypt(padding, encrypted_key) {
                    Ok(symmetric_key) => {
                        println!(
                            "✓ Successfully decrypted symmetric key (length: {})",
                            symmetric_key.len()
                        );
                        return Ok(symmetric_key);
                    }
                    Err(e) => {
                        log_encryption_error(&format!(
                            "RSA decryption failed for key at position {}: {}",
                            key_start, e
                        ));
                        // Continue searching for other potential keys
                        continue;
                    }
                }
            }
        }
    }

    // Alternative: look for 04 81 patterns (OCTET STRING with length < 256)
    // Some keys might be shorter
    for i in 0..data.len().saturating_sub(3) {
        if data[i] == 0x04 && data[i + 1] == 0x81 {
            let key_length = data[i + 2] as usize;
            if key_length >= 200 && key_length <= 256 {
                // Reasonable range for RSA-2048 encrypted data
                let key_start = i + 3;
                let key_end = key_start + key_length;

                if key_end <= data.len() {
                    let encrypted_key = &data[key_start..key_end];
                    println!(
                        "Found alternative RSA-encrypted key at position {}, length: {}",
                        key_start,
                        encrypted_key.len()
                    );

                    let padding = Oaep::new::<Sha256>();
                    match private_key.decrypt(padding, encrypted_key) {
                        Ok(symmetric_key) => {
                            println!(
                                "✓ Successfully decrypted alternative symmetric key (length: {})",
                                symmetric_key.len()
                            );
                            return Ok(symmetric_key);
                        }
                        Err(e) => {
                            log_encryption_error(&format!(
                                "RSA decryption failed for alternative key at position {}: {}",
                                key_start, e
                            ));
                            continue;
                        }
                    }
                }
            }
        }
    }

    // Final fallback: look for any OCTET STRING that might contain the key
    // This is more aggressive and tries various length encodings
    for i in 0..data.len().saturating_sub(200) {
        if data[i] == 0x04 {
            let mut key_start = i + 1;
            let mut key_length = 0usize;

            // Parse length encoding
            if i + 1 < data.len() {
                if data[i + 1] & 0x80 == 0 {
                    // Single byte length
                    key_length = data[i + 1] as usize;
                    key_start = i + 2;
                } else if data[i + 1] == 0x82 && i + 3 < data.len() {
                    // Two byte length
                    key_length = ((data[i + 2] as usize) << 8) | (data[i + 3] as usize);
                    key_start = i + 4;
                } else if data[i + 1] == 0x81 && i + 2 < data.len() {
                    // One byte length
                    key_length = data[i + 2] as usize;
                    key_start = i + 3;
                }

                // Only try if length seems reasonable for RSA-encrypted data
                if key_length >= 200 && key_length <= 512 {
                    let key_end = key_start + key_length;
                    if key_end <= data.len() {
                        let encrypted_key = &data[key_start..key_end];

                        let padding = Oaep::new::<Sha256>();
                        match private_key.decrypt(padding, encrypted_key) {
                            Ok(symmetric_key) => {
                                println!(
                                    "✓ Successfully decrypted fallback symmetric key at position {} (length: {})",
                                    key_start,
                                    symmetric_key.len()
                                );
                                return Ok(symmetric_key);
                            }
                            Err(_) => {
                                // Silently continue for fallback attempts
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to extract and decrypt RSA-encrypted symmetric key from AWS KMS PKCS#7 ciphertext"
    ))
}

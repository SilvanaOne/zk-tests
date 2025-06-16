use crate::attestation::get_kms_attestation;
use crate::encrypt::{decrypt_kms_ciphertext, generate_kms_key_pair};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Result, anyhow};
use aws_config::BehaviorVersion;
use aws_sdk_kms::{
    Client,
    primitives::Blob,
    types::{KeyEncryptionMechanism, RecipientInfo},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info};

/// Encrypted data container that includes both the encrypted data key and the encrypted content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The plaintext data key from KMS (AES-256 symmetric key, stored for verification)
    pub plaintext_data_key: Vec<u8>,
    /// The encrypted data key from KMS (ciphertext blob for storage/later decryption)
    pub encrypted_data_key: Vec<u8>,
    /// The actual data encrypted with the plaintext data key
    pub encrypted_content: Vec<u8>,
    /// The nonce used for AES-GCM encryption
    pub nonce: Vec<u8>,
}

#[derive(Clone)]
pub struct KMS {
    client: Arc<Client>,
    key_id: String,
}

impl KMS {
    pub async fn new(key_id: impl Into<String>) -> Result<Self> {
        println!("Initializing KMS...");
        let shared_cfg = aws_config::defaults(BehaviorVersion::latest()).load().await;
        println!("Creating KMS client...");
        let client = Client::new(&shared_cfg);
        println!("KMS: Client created");

        // println!("Resolving key ID...");
        // // Resolve the key name to actual key ID
        // let key_id = Self::resolve_key_id(&client, &key_name).await?;
        // println!("Key ID resolved: {}", key_id);

        Ok(Self {
            client: Arc::new(client),
            key_id: key_id.into(),
        })
    }

    /// Static method to resolve a key name/alias to its actual key ID
    /// Handles various input formats:
    /// - "my-key" -> "alias/my-key" -> actual key ID
    /// - "alias/my-key" -> actual key ID  
    /// - "arn:aws:kms:..." -> actual key ID
    /// - actual key ID -> returns as-is (after validation)
    // pub async fn resolve_key_id(client: &Client, key_identifier: &str) -> Result<String> {
    //     // If it looks like a plain name (no special prefixes), prepend "alias/"
    //     let key_to_describe = if !key_identifier.starts_with("alias/")
    //         && !key_identifier.starts_with("arn:")
    //         && !key_identifier
    //             .chars()
    //             .all(|c| c.is_ascii_hexdigit() || c == '-')
    //         && key_identifier.len() < 36
    //     // UUID-like key IDs are typically 36 chars
    //     {
    //         format!("alias/{}", key_identifier)
    //     } else {
    //         key_identifier.to_string()
    //     };

    //     // Use describe_key to get the actual key information
    //     let response = client
    //         .describe_key()
    //         .key_id(&key_to_describe)
    //         .send()
    //         .await
    //         .map_err(|e| anyhow!("Failed to describe KMS key '{}': {}", key_to_describe, e))?;

    //     let key_metadata = response
    //         .key_metadata()
    //         .ok_or_else(|| anyhow!("No key metadata returned for key '{}'", key_to_describe))?;

    //     let key_id = key_metadata.key_id();

    //     Ok(key_id.to_string())
    // }

    /// Encrypt data using standard KMS data key
    /// Uses "kms:GenerateDataKey*" policy (standard, not Nitro Enclaves)
    pub async fn encrypt(&self, data: &[u8]) -> Result<EncryptedData> {
        // Generate a data key from KMS using standard flow
        println!("KMS: Generating data key (standard)...");
        let data_key_response = self
            .client
            .generate_data_key()
            .key_id(&self.key_id)
            .key_spec(aws_sdk_kms::types::DataKeySpec::Aes256)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to generate KMS data key: {}", e))?;

        // Extract the plaintext and encrypted data key (standard KMS flow)
        let plaintext_key = data_key_response
            .plaintext()
            .ok_or_else(|| anyhow!("No plaintext key returned from KMS"))?;

        let encrypted_data_key = data_key_response
            .ciphertext_blob()
            .ok_or_else(|| anyhow!("No encrypted data key returned from KMS"))?;

        // Debug: log the plaintext key for verification
        println!("Encryption - Plaintext key: {:?}", plaintext_key.as_ref());

        // Use the plaintext key for AES-GCM encryption
        let cipher = Aes256Gcm::new_from_slice(plaintext_key.as_ref())
            .map_err(|e| anyhow!("Failed to create AES cipher: {}", e))?;

        // Generate a random nonce
        let mut nonce_bytes = [0u8; 12]; // 96-bit nonce for GCM
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        println!("Encryption - Nonce: {:?}", nonce_bytes);

        // Encrypt the data
        let encrypted_content = cipher
            .encrypt(nonce, data)
            .map_err(|e| anyhow!("Failed to encrypt data: {}", e))?;

        println!(
            "KMS: Data encrypted with key: {:?}",
            hex::encode(plaintext_key.as_ref())
        );

        Ok(EncryptedData {
            plaintext_data_key: plaintext_key.as_ref().to_vec(),
            encrypted_data_key: encrypted_data_key.as_ref().to_vec(),
            encrypted_content,
            nonce: nonce_bytes.to_vec(),
        })
    }

    /// Decrypt data using KMS
    /// Uses "kms:Decrypt" policy with Nitro Enclaves attestation
    pub async fn decrypt(&self, encrypted_data: &EncryptedData) -> Result<Vec<u8>> {
        // Decrypt the data key using KMS with Nitro Enclaves attestation
        println!("KMS: Decrypting data key...");
        let pair = match generate_kms_key_pair() {
            Ok(pair) => pair,
            Err(e) => {
                println!("KMS: Failed to generate KMS key pair: {:?}", e);
                return Err(anyhow!("Failed to generate KMS key pair: {:?}", e));
            }
        };
        let attestation = match get_kms_attestation(&pair.public_key) {
            Ok(attestation) => attestation,
            Err(e) => {
                println!("KMS: Failed to get KMS attestation: {:?}", e);
                return Err(anyhow!("Failed to get KMS attestation: {:?}", e));
            }
        };
        let recipient = RecipientInfo::builder()
            .attestation_document(Blob::new(attestation))
            .key_encryption_algorithm(KeyEncryptionMechanism::RsaesOaepSha256)
            .build();

        let decrypt_response = match self
            .client
            .decrypt()
            .ciphertext_blob(Blob::new(encrypted_data.encrypted_data_key.clone()))
            .recipient(recipient)
            .send()
            .await
        {
            Ok(decrypt_response) => decrypt_response,
            Err(e) => {
                println!("KMS: Failed to decrypt KMS data key E302: {:?}", e);
                return Err(anyhow!("Failed to decrypt KMS data key E302: {:?}", e));
            }
        };
        println!("Decrypt response: {:?}", decrypt_response);

        // Extract the ciphertext for recipient and decrypt it manually
        let ciphertext_for_recipient = match decrypt_response.ciphertext_for_recipient {
            Some(ciphertext_for_recipient) => ciphertext_for_recipient,
            None => {
                println!("KMS: No ciphertext for recipient returned from KMS decrypt");
                return Err(anyhow!(
                    "No ciphertext for recipient returned from KMS decrypt"
                ));
            }
        };
        println!("Ciphertext for recipient: {:?}", ciphertext_for_recipient);

        let plaintext_key =
            match decrypt_kms_ciphertext(ciphertext_for_recipient.as_ref(), &pair.private_key) {
                Ok(plaintext_key) => plaintext_key,
                Err(e) => {
                    println!("KMS: Failed to decrypt KMS data key E303: {:?}", e);
                    return Err(anyhow!("Failed to decrypt KMS data key E303: {:?}", e));
                }
            };

        self.complete_decryption(encrypted_data, plaintext_key).await
    }

    /// Helper method to complete the decryption process with the AES key
    async fn complete_decryption(&self, encrypted_data: &EncryptedData, plaintext_key: Vec<u8>) -> Result<Vec<u8>> {
        println!(
            "Decryption - Decrypted data key: {:?}",
            hex::encode(&plaintext_key)
        );
        println!(
            "Decryption - Stored plaintext data key: {:?}",
            hex::encode(&encrypted_data.plaintext_data_key)
        );

        // Verify that the decrypted key matches the plaintext data key from original encryption
        if plaintext_key == encrypted_data.plaintext_data_key {
            info!(
                "✓ Key verification successful: KMS decrypted key matches stored plaintext data key"
            );
            debug!(
                "Key match - Stored plaintext: {}, KMS decrypted: {}",
                hex::encode(&encrypted_data.plaintext_data_key),
                hex::encode(&plaintext_key)
            );
        } else {
            error!(
                "✗ Key verification FAILED: KMS decrypted key does not match stored plaintext data key"
            );
            error!(
                "  Expected (stored plaintext): {}",
                hex::encode(&encrypted_data.plaintext_data_key)
            );
            error!(
                "  Got (KMS decrypted):         {}",
                hex::encode(&plaintext_key)
            );
            return Err(anyhow!(
                "Key verification failed: KMS decrypted AES key does not match stored plaintext data key"
            ));
        }

        // Use the decrypted key for AES-GCM decryption
        let cipher = match Aes256Gcm::new_from_slice(plaintext_key.as_ref())
            .map_err(|e| anyhow!("Failed to create AES cipher: {}", e))
        {
            Ok(cipher) => cipher,
            Err(e) => {
                println!("KMS: Failed to create AES cipher: {:?}", e);
                return Err(anyhow!("Failed to create AES cipher: {:?}", e));
            }
        };

        // Reconstruct the nonce
        if encrypted_data.nonce.len() != 12 {
            return Err(anyhow!(
                "Invalid nonce length: expected 12 bytes, got {}",
                encrypted_data.nonce.len()
            ));
        }
        let nonce = Nonce::from_slice(&encrypted_data.nonce);

        println!("Decryption - Nonce: {:?}", encrypted_data.nonce);
        println!(
            "Decryption - Encrypted content length: {}",
            encrypted_data.encrypted_content.len()
        );

        // Decrypt the data
        let decrypted_data = match cipher.decrypt(nonce, encrypted_data.encrypted_content.as_ref())
        {
            Ok(decrypted_data) => decrypted_data,
            Err(e) => {
                println!("KMS: Failed to decrypt data: {:?}", e);
                return Err(anyhow!("Failed to decrypt data: {:?}", e));
            }
        };

        println!("KMS: Data decrypted");

        Ok(decrypted_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new() {
        // Note: This test requires AWS credentials and a valid KMS key
        // Skip in CI or when credentials are not available
        // dotenvy::dotenv().ok();
        if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
            return;
        }
        if std::env::var("KMS_KEY_ID").is_err() {
            return;
        }
        let key_name = std::env::var("KMS_KEY_NAME").unwrap();

        // Test creating KMS instance from simple key name
        let kms = KMS::new(key_name).await.unwrap();
        let data = b"Hello, world!";

        let encrypted = kms.encrypt(data).await.unwrap();
        let decrypted = kms.decrypt(&encrypted).await.unwrap();

        assert_eq!(data, decrypted.as_slice());
    }
}

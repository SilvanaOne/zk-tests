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

/// Encrypted data container that includes both the encrypted data key and the encrypted content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// The data key encrypted by KMS
    pub encrypted_data_key: Vec<u8>,
    /// The actual data encrypted with the data key
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
    /// Create a new KMS instance by resolving a key name/alias to its key ID
    /// Accepts key names like "my-seed-key" and converts them to "alias/my-seed-key"
    /// Also accepts full aliases like "alias/my-seed-key" or actual key IDs
    pub async fn new(key_id: impl Into<String>) -> Result<Self> {
        println!("Initializing KMS...");
        // let key_name = key_name.into();
        // println!("Creating KMS client...");
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

    /// Encrypt data using KMS data key with Nitro Enclaves
    /// Uses "kms:GenerateDataKey*" policy with attestation
    pub async fn encrypt(&self, data: &[u8]) -> Result<EncryptedData> {
        // Generate a data key from KMS using Nitro Enclaves attestation
        println!("KMS: Generating data key with Nitro Enclaves attestation...");
        let pair = generate_kms_key_pair()?;
        let attestation = match get_kms_attestation(&pair.public_key) {
            Ok(attestation) => attestation,
            Err(e) => {
                println!("KMS: Failed to get KMS attestation for encryption: {:?}", e);
                return Err(anyhow!(
                    "Failed to get KMS attestation for encryption: {:?}",
                    e
                ));
            }
        };

        let recipient = RecipientInfo::builder()
            .attestation_document(Blob::new(attestation))
            .key_encryption_algorithm(KeyEncryptionMechanism::RsaesOaepSha256)
            .build();

        let data_key_response = self
            .client
            .generate_data_key()
            .key_id(&self.key_id)
            .key_spec(aws_sdk_kms::types::DataKeySpec::Aes256)
            .recipient(recipient)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to generate KMS data key with attestation: {}", e))?;

        // Extract the encrypted data key (for storage)
        let encrypted_data_key = data_key_response
            .ciphertext_blob()
            .ok_or_else(|| anyhow!("No encrypted data key returned from KMS"))?;

        // Extract the plaintext key encrypted for our enclave
        let ciphertext_for_recipient = data_key_response
            .ciphertext_for_recipient()
            .ok_or_else(|| anyhow!("No ciphertext for recipient returned from KMS"))?;

        // Decrypt the symmetric key from the PKCS#7 structure
        let plaintext_key =
            decrypt_kms_ciphertext(ciphertext_for_recipient.as_ref(), &pair.private_key)
                .map_err(|e| anyhow!("Failed to decrypt data key during encryption: {}", e))?;

        // Debug: log the plaintext key for verification
        println!("Encryption - Plaintext key: {:?}", plaintext_key);

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

        println!("KMS: Data encrypted");

        Ok(EncryptedData {
            encrypted_data_key: encrypted_data_key.as_ref().to_vec(),
            encrypted_content,
            nonce: nonce_bytes.to_vec(),
        })
    }

    /// Decrypt data using KMS
    /// Uses "kms:Decrypt" policy
    pub async fn decrypt(&self, encrypted_data: &EncryptedData) -> Result<Vec<u8>> {
        // Decrypt the data key using KMS
        println!("KMS: Decrypting data key...");
        let pair = generate_kms_key_pair()?;
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
        println!("Recipient: {:?}", recipient);
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

        // Extract the plaintext data key
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
        println!("Decryption - Plaintext key: {:?}", plaintext_key);

        // let plaintext_key = decrypt_response
        //     .plaintext()
        //     .ok_or_else(|| anyhow!("No plaintext key returned from KMS decrypt"))?;

        // Use the decrypted key for AES-GCM decryption
        let cipher = Aes256Gcm::new_from_slice(plaintext_key.as_ref())
            .map_err(|e| anyhow!("Failed to create AES cipher: {}", e))?;

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
        let decrypted_data = cipher
            .decrypt(nonce, encrypted_data.encrypted_content.as_ref())
            .map_err(|e| anyhow!("Failed to decrypt data: {}", e))?;

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

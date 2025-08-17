use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::{Client, primitives::Blob, types::AttributeValue};
use kms::{EncryptedData, KMS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// Key for identifying a unique login
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct LoginKey {
    pub login_type: String,
    pub login: String,
}

/// Value stored in the database
#[derive(Debug, Serialize, Deserialize)]
pub struct KeypairValue {
    pub address: String,
    pub encrypted_private_key: Vec<u8>,  // The sui_private_key encrypted
    pub created_at: i64,
}

pub struct SecureKeyStorage {
    client: Arc<Client>,
    table_name: String,
    kms: Arc<KMS>,
}

impl SecureKeyStorage {
    pub async fn new(table_name: String, kms_key_id: String) -> Result<Self> {
        // Reuse the global DynamoDB client if available
        let client = if let Some(client) = super::DYNAMODB_CLIENT.get() {
            debug!("Reusing existing DynamoDB client for secure storage");
            client.clone()
        } else {
            info!("Initializing new DynamoDB client for secure storage");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let new_client = Arc::new(Client::new(&config));
            
            // Try to set the client
            match super::DYNAMODB_CLIENT.set(new_client.clone()) {
                Ok(_) => {
                    info!("DynamoDB client initialized for secure storage");
                    new_client
                }
                Err(_) => {
                    // Another thread initialized it first
                    super::DYNAMODB_CLIENT.get()
                        .ok_or_else(|| anyhow!("DynamoDB client not initialized despite concurrent set"))?
                        .clone()
                }
            }
        };
        
        let kms = Arc::new(KMS::new(kms_key_id).await?);
        
        Ok(Self {
            client,
            table_name,
            kms,
        })
    }
    
    /// Get existing keypair or return None if not found
    pub async fn get_keypair_address(
        &self,
        login_type: &str,
        login: &str,
    ) -> Result<Option<String>> {
        let key = LoginKey {
            login_type: login_type.to_string(),
            login: login.to_string(),
        };
        
        let primary_key = self.create_primary_key(&key)?;
        
        if let Some(existing) = self.get_keypair(&primary_key).await? {
            info!("Found existing keypair for {}:{}", login_type, login);
            return Ok(Some(existing.address));
        }
        
        Ok(None)
    }
    
    /// Store a new keypair
    pub async fn store_new_keypair(
        &self,
        login_type: &str,
        login: &str,
        address: &str,
        private_key: &str,
    ) -> Result<()> {
        let key = LoginKey {
            login_type: login_type.to_string(),
            login: login.to_string(),
        };
        
        let primary_key = self.create_primary_key(&key)?;
        
        // Encrypt the private key
        let encrypted_private_key = self.kms.encrypt(private_key.as_bytes()).await?;
        
        // Store in database
        let value = KeypairValue {
            address: address.to_string(),
            encrypted_private_key: bincode::serialize(&encrypted_private_key)?,
            created_at: chrono::Utc::now().timestamp(),
        };
        
        self.store_keypair(&primary_key, &value).await?;
        info!("Stored new keypair for {}:{} with address: {}", login_type, login, address);
        
        Ok(())
    }
    
    /// Get the private key for a given login (for internal use only)
    pub async fn get_private_key(
        &self,
        login_type: &str,
        login: &str,
    ) -> Result<String> {
        let key = LoginKey {
            login_type: login_type.to_string(),
            login: login.to_string(),
        };
        
        let primary_key = self.create_primary_key(&key)?;
        
        let value = self.get_keypair(&primary_key).await?
            .ok_or_else(|| anyhow!("Keypair not found for {}:{}", login_type, login))?;
        
        // Decrypt the private key
        let encrypted_data: EncryptedData = bincode::deserialize(&value.encrypted_private_key)?;
        let decrypted = self.kms.decrypt(&encrypted_data).await?;
        
        Ok(String::from_utf8(decrypted)?)
    }
    
    fn create_primary_key(&self, key: &LoginKey) -> Result<Vec<u8>> {
        bincode::serialize(key).map_err(|e| anyhow!("Failed to serialize key: {}", e))
    }
    
    async fn get_keypair(&self, primary_key: &[u8]) -> Result<Option<KeypairValue>> {
        let key = HashMap::from([
            ("id".to_string(), AttributeValue::B(Blob::new(primary_key.to_vec()))),
        ]);
        
        let result = self.client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(key))
            .send()
            .await?;
        
        if let Some(item) = result.item {
            let value_attr = item.get("value")
                .ok_or_else(|| anyhow!("Missing 'value' attribute"))?;
            
            let value_blob = value_attr.as_b()
                .map_err(|_| anyhow!("Expected binary attribute for value"))?;
            
            let value: KeypairValue = bincode::deserialize(value_blob.as_ref())?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
    
    async fn store_keypair(&self, primary_key: &[u8], value: &KeypairValue) -> Result<()> {
        let value_bytes = bincode::serialize(value)?;
        
        let mut item = HashMap::new();
        item.insert("id".to_string(), AttributeValue::B(Blob::new(primary_key.to_vec())));
        item.insert("value".to_string(), AttributeValue::B(Blob::new(value_bytes)));
        item.insert("created_at".to_string(), AttributeValue::N(value.created_at.to_string()));
        
        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;
        
        info!("Keypair stored successfully for address: {}", value.address);
        Ok(())
    }
}
use crate::db::{Key, Value};
use crate::kms::{EncryptedData, KMS};
use anyhow::{Result, anyhow};
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::{
    Client,
    primitives::Blob,
    types::{AttributeValue, ReturnValue},
};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct DynamoDB {
    client: Arc<Client>,
    table: String,
    kms: Arc<KMS>,
}

impl DynamoDB {
    pub async fn new(table: impl Into<String>, key_name: impl Into<String>) -> Result<Self> {
        println!("Initializing DynamoDB...");
        let shared_cfg = aws_config::defaults(BehaviorVersion::latest()).load().await;

        println!("Creating DynamoDB client...");
        let client = Client::new(&shared_cfg);

        println!("Initializing KMS...");
        let kms = KMS::new(key_name)
            .await
            .map_err(|e| anyhow!("Failed to initialize KMS: {}", e))?;

        println!("Creating DynamoDB instance...");
        Ok(Self {
            client: Arc::new(client),
            table: table.into(),
            kms: Arc::new(kms),
        })
    }

    /// Put a brand-new item: { id (PK, B) , value (B) , nonce (N) }
    pub async fn create(&self, id: &Key, value: &Value, nonce: u64) -> Result<()> {
        let mut item = HashMap::new();
        item.insert("id".into(), AttributeValue::B(self.key_to_blob(id)?));
        item.insert(
            "value".into(),
            AttributeValue::B(self.value_to_blob(value).await?),
        );
        item.insert("nonce".into(), AttributeValue::N(nonce.to_string()));

        self.client
            .put_item()
            .table_name(&self.table)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| anyhow!("DynamoDB create operation failed: {}", e))?;
        Ok(())
    }

    /// Update `nonce`.
    pub async fn update(&self, id: &Key, nonce: u64) -> Result<()> {
        // key ---------------------------------------------------
        let mut key = HashMap::new();
        key.insert("id".into(), AttributeValue::B(self.key_to_blob(id)?));

        // expression pieces -------------------------------------
        let mut expr_names = HashMap::new();
        expr_names.insert("#n".into(), "nonce".into());

        let mut expr_values = HashMap::new();
        let mut update_parts = Vec::new();

        expr_values.insert(":new_nonce".into(), AttributeValue::N(nonce.to_string()));
        update_parts.push("#n = :new_nonce");

        self.client
            .update_item()
            .table_name(&self.table)
            .set_key(Some(key))
            .update_expression(format!("SET {}", update_parts.join(", ")))
            .set_expression_attribute_names(Some(expr_names))
            .set_expression_attribute_values(Some(expr_values))
            .return_values(ReturnValue::AllNew)
            .send()
            .await
            .map_err(|e| anyhow!("DynamoDB update operation failed: {}", e))?;
        Ok(())
    }

    /// Read the row back and decode its two attributes.
    pub async fn read(&self, id: &Key) -> Result<Option<(Value, u64)>> {
        let key = [("id".into(), AttributeValue::B(self.key_to_blob(id)?))]
            .into_iter()
            .collect::<HashMap<_, _>>();

        let resp = self
            .client
            .get_item()
            .table_name(&self.table)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| anyhow!("DynamoDB read operation failed: {}", e))?;

        if let Some(item) = resp.item {
            let value_attr = item
                .get("value")
                .ok_or_else(|| anyhow!("Missing 'value' attribute in DynamoDB item"))?;
            let value = self.blob_to_value(value_attr).await?;

            let nonce_attr = item
                .get("nonce")
                .ok_or_else(|| anyhow!("Missing 'nonce' attribute in DynamoDB item"))?;
            let nonce_str = nonce_attr
                .as_n()
                .map_err(|_| anyhow!("Expected numeric attribute for nonce"))?;
            let nonce = nonce_str
                .parse::<u64>()
                .map_err(|e| anyhow!("Failed to parse nonce as u64: {}", e))?;

            Ok(Some((value, nonce)))
        } else {
            Ok(None)
        }
    }

    /// Helpers – convert Key Value to/from the SDK's Blob type.
    fn key_to_blob(&self, key: &Key) -> Result<Blob> {
        let serialized =
            bincode::serialize(key).map_err(|e| anyhow!("Failed to serialize key: {}", e))?;
        Ok(Blob::new(serialized))
    }

    async fn value_to_blob(&self, value: &Value) -> Result<Blob> {
        let serialized =
            bincode::serialize(value).map_err(|e| anyhow!("Failed to serialize value: {}", e))?;
        let encrypted_data = self.kms.encrypt(&serialized).await?;
        let serialized_encrypted_data = bincode::serialize(&encrypted_data)
            .map_err(|e| anyhow!("Failed to serialize encrypted data: {}", e))?;
        Ok(Blob::new(serialized_encrypted_data))
    }

    async fn blob_to_value(&self, av: &AttributeValue) -> Result<Value> {
        let blob = av
            .as_b()
            .map_err(|_| anyhow!("Expected binary attribute for value"))?;
        let encrypted_data = bincode::deserialize::<EncryptedData>(blob.as_ref())
            .map_err(|e| anyhow!("Failed to deserialize encrypted data: {}", e))?;
        let decrypted_data = self.kms.decrypt(&encrypted_data).await?;
        let value = bincode::deserialize::<Value>(&decrypted_data)
            .map_err(|e| anyhow!("Failed to deserialize value: {}", e))?;
        Ok(value)
    }
}

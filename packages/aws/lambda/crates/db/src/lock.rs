use anyhow::{anyhow, Result};
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use std::sync::Arc;
use std::error::Error;
use tracing::{debug, info, warn, error};

pub struct KeyLock {
    client: Arc<aws_sdk_dynamodb::Client>,
    table_name: String,
}

impl KeyLock {
    pub async fn new() -> Result<Self> {
        // Get or initialize the DynamoDB client (reused across invocations)
        let client = if let Some(client) = super::DYNAMODB_CLIENT.get() {
            debug!("Reusing existing DynamoDB client from previous invocation");
            client.clone()
        } else {
            info!("Initializing new DynamoDB client for first invocation");
            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let new_client = Arc::new(aws_sdk_dynamodb::Client::new(&config));
            
            // Try to set the client, but if another thread beat us to it, use theirs
            match super::DYNAMODB_CLIENT.set(new_client.clone()) {
                Ok(_) => {
                    info!("DynamoDB client initialized and cached");
                    new_client
                }
                Err(_) => {
                    // Another thread initialized it first, use that one
                    debug!("Another thread initialized DynamoDB client, using that");
                    super::DYNAMODB_CLIENT.get().unwrap().clone()
                }
            }
        };
        
        let table_name = std::env::var("LOCKS_TABLE_NAME")
            .unwrap_or_else(|_| "sui-key-locks".to_string());
        
        info!("KeyLock initialized with table: {}", table_name);
        
        Ok(Self {
            client,
            table_name,
        })
    }

    pub async fn acquire_lock(&self, address: &str, chain: &str) -> Result<LockGuard> {
        let mut retry_count = 0;
        let max_retries = 2;
        let start_time = Utc::now();
        
        loop {
            // Check if we've exceeded the timeout
            let elapsed = Utc::now() - start_time;
            let timeout = Duration::seconds(10);
            if elapsed > timeout {
                let elapsed_ms = elapsed.num_milliseconds();
                error!("Failed to acquire lock after {}ms - timeout exceeded", elapsed_ms);
                return Err(anyhow!("Lock acquisition timeout after 10 seconds"));
            }
            
            let lock_id = format!("{}#{}", address, chain);
            let now = Utc::now();
            let expiry = now + Duration::seconds(60);
            
            let elapsed_ms = elapsed.num_milliseconds();
            debug!("Attempting to acquire lock for {} (attempt {}, elapsed: {}ms)", 
                   lock_id, retry_count + 1, elapsed_ms);
            
            let mut item = HashMap::new();
            item.insert(
                "address".to_string(),
                AttributeValue::S(address.to_string()),
            );
            item.insert(
                "chain".to_string(),
                AttributeValue::S(chain.to_string()),
            );
            item.insert(
                "locked_at".to_string(),
                AttributeValue::S(now.to_rfc3339()),
            );
            item.insert(
                "expires_at".to_string(),
                AttributeValue::N(expiry.timestamp().to_string()),
            );
            item.insert(
                "lock_id".to_string(),
                AttributeValue::S(lock_id.clone()),
            );
            
            // Try to acquire lock with conditional put (only if item doesn't exist)
            debug!("Calling DynamoDB put_item on table: {}", self.table_name);
            let result = self.client
                .put_item()
                .table_name(&self.table_name)
                .set_item(Some(item))
                .condition_expression("attribute_not_exists(address)")
                .send()
                .await;
            
            match result {
                Ok(_) => {
                    let total_elapsed = Utc::now() - start_time;
                    let elapsed_ms = total_elapsed.num_milliseconds();
                    info!("Lock acquired for {} after {}ms (attempt {})", 
                          lock_id, elapsed_ms, retry_count + 1);
                    return Ok(LockGuard {
                        client: self.client.clone(),
                        table_name: self.table_name.clone(),
                        address: address.to_string(),
                        chain: chain.to_string(),
                        acquired_at: now,
                        released: false,
                    });
                }
                Err(e) => {
                    // Check if lock exists and is expired
                    // Need to check both the error string and the actual error type
                    // The SDK wraps the error in ServiceError, but the string still contains the error name
                    let error_str = format!("{:?}", e);
                    let is_conditional_check_failed = error_str.contains("ConditionalCheckFailedException");
                    
                    if is_conditional_check_failed {
                        // Try to check if existing lock is expired
                        if retry_count < max_retries && self.is_lock_expired(address, chain).await.unwrap_or(false) {
                            // Try to delete expired lock and retry
                            let elapsed_ms = (Utc::now() - start_time).num_milliseconds();
                            warn!("Found expired lock for {}, attempting to clean up (elapsed: {}ms)", 
                                  lock_id, elapsed_ms);
                            if self.delete_lock(address, chain).await.is_ok() {
                                // Retry acquiring lock with backoff
                                let backoff_ms = 100 * (retry_count + 1);
                                debug!("Waiting {}ms before retry", backoff_ms);
                                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms as u64)).await;
                                retry_count += 1;
                                continue; // Loop to retry
                            }
                        }
                        
                        // If we can't get the lock, wait a bit before failing
                        // This helps with contention scenarios
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        
                        error!("Failed to acquire lock for {} - lock is held by another instance", lock_id);
                        return Err(anyhow!("Lock is currently held by another Lambda instance"));
                    } else {
                        error!("Failed to acquire lock for {} - service error: {}", lock_id, e);
                        error!("Error details: {:?}", e);
                        error!("Error source: {:?}", e.source());
                        return Err(anyhow!("Failed to acquire lock: service error"));
                    }
                }
            }
        }
    }
    
    async fn is_lock_expired(&self, address: &str, chain: &str) -> Result<bool> {
        let result = self.client
            .get_item()
            .table_name(&self.table_name)
            .key("address", AttributeValue::S(address.to_string()))
            .key("chain", AttributeValue::S(chain.to_string()))
            .send()
            .await?;
        
        if let Some(item) = result.item {
            if let Some(AttributeValue::N(expiry_str)) = item.get("expires_at") {
                let expiry_timestamp = expiry_str.parse::<i64>()?;
                let now = Utc::now().timestamp();
                return Ok(now > expiry_timestamp);
            }
        }
        
        Ok(false)
    }
    
    async fn delete_lock(&self, address: &str, chain: &str) -> Result<()> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("address", AttributeValue::S(address.to_string()))
            .key("chain", AttributeValue::S(chain.to_string()))
            .send()
            .await?;
        
        debug!("Lock deleted for {}#{}", address, chain);
        Ok(())
    }
}

pub struct LockGuard {
    client: Arc<aws_sdk_dynamodb::Client>,
    table_name: String,
    address: String,
    chain: String,
    acquired_at: DateTime<Utc>,
    released: bool,
}

impl LockGuard {
    pub async fn release(mut self) -> Result<()> {
        debug!("Releasing lock for {}#{}", self.address, self.chain);
        
        let result = self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("address", AttributeValue::S(self.address.clone()))
            .key("chain", AttributeValue::S(self.chain.clone()))
            .send()
            .await;
        
        let held_duration = Utc::now() - self.acquired_at;
        let held_ms = held_duration.num_milliseconds();
        
        // Mark as released to prevent Drop from logging
        self.released = true;
        
        match result {
            Ok(_) => {
                info!("Lock released for {}#{} after {}ms", self.address, self.chain, held_ms);
                Ok(())
            }
            Err(e) => {
                error!("Failed to release lock for {}#{}: {}", self.address, self.chain, e);
                Err(anyhow!("Failed to release lock: {}", e))
            }
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Only log if the lock wasn't explicitly released
        if !self.released {
            let held_duration = Utc::now() - self.acquired_at;
            let held_ms = held_duration.num_milliseconds();
            warn!(
                "LockGuard for {}#{} dropped without explicit release after {}ms",
                self.address, self.chain, held_ms
            );
        }
    }
}
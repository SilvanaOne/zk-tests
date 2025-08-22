use anyhow::Result;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sui_rpc::proto::sui::rpc::v2beta2 as proto;
use sui_rpc::Client as GrpcClient;
use sui_sdk_types as sui;
use parking_lot::Mutex;

const MIN_BALANCE: u64 = 100_000_000;
const MAX_RETRIES: u32 = 6;
const RETRY_DELAY_MS: u64 = 500;

/// RAII-style guard returned by `try_lock_coin`.
/// When this guard is dropped, the coin lock is automatically released.
pub struct CoinLockGuard {
    manager: CoinLockManager,
    coin_id: sui::Address,
}

impl Drop for CoinLockGuard {
    fn drop(&mut self) {
        self.manager.release_coin(self.coin_id);
    }
}

/// Coin lock manager to prevent concurrent usage of the same coin
#[derive(Clone)]
pub struct CoinLockManager {
    locks: Arc<Mutex<HashMap<sui::Address, Instant>>>,
    lock_timeout: Duration,
}

impl CoinLockManager {
    pub fn new(lock_timeout_seconds: u64) -> Self {
        Self {
            locks: Arc::new(Mutex::new(HashMap::new())),
            lock_timeout: Duration::from_secs(lock_timeout_seconds),
        }
    }

    /// Attempts to lock a coin for exclusive use.
    /// Returns `Some(CoinLockGuard)` if the coin was successfully locked; `None` otherwise.
    pub fn try_lock_coin(&self, coin_id: sui::Address) -> Option<CoinLockGuard> {
        let mut locks = self.locks.lock();

        // Clean up expired locks first
        let now = Instant::now();
        locks.retain(|_, lock_time| now.duration_since(*lock_time) < self.lock_timeout);

        use std::collections::hash_map::Entry;
        match locks.entry(coin_id) {
            Entry::Occupied(_) => None, // already locked
            Entry::Vacant(entry) => {
                entry.insert(now);
                Some(CoinLockGuard {
                    manager: self.clone(),
                    coin_id,
                })
            }
        }
    }

    /// Releases a coin lock
    fn release_coin(&self, coin_id: sui::Address) {
        let mut locks = self.locks.lock();
        locks.remove(&coin_id);
    }

    /// Checks if a coin is currently locked
    #[allow(dead_code)]
    pub fn is_locked(&self, coin_id: sui::Address) -> bool {
        let mut locks = self.locks.lock();

        // Clean up expired locks first
        let now = Instant::now();
        locks.retain(|_, lock_time| now.duration_since(*lock_time) < self.lock_timeout);

        locks.contains_key(&coin_id)
    }
}

/// Global coin lock manager instance
static COIN_LOCK_MANAGER: std::sync::OnceLock<CoinLockManager> = std::sync::OnceLock::new();

pub fn get_coin_lock_manager() -> &'static CoinLockManager {
    COIN_LOCK_MANAGER.get_or_init(|| CoinLockManager::new(30)) // 30 seconds timeout
}

#[derive(Debug, Clone)]
pub struct CoinInfo {
    pub object_ref: sui::ObjectReference,
    pub balance: u64,
}

impl CoinInfo {
    pub fn object_id(&self) -> sui::Address {
        *self.object_ref.object_id()
    }
}

/// Fetches a coin with sufficient balance and locks it for exclusive use
pub async fn fetch_coin(
    rpc_url: &str,
    sender: sui::Address,
    min_balance: u64,
) -> Result<Option<(CoinInfo, CoinLockGuard)>> {
    let lock_manager = get_coin_lock_manager();
    
    for attempt in 1..=MAX_RETRIES {
        let mut client = GrpcClient::new(rpc_url.to_string())?;
        let mut live = client.live_data_client();
        
        // List owned objects to find SUI coins
        let resp = live
            .list_owned_objects(proto::ListOwnedObjectsRequest {
                owner: Some(sender.to_string()),
                page_size: Some(100),
                page_token: None,
                read_mask: Some(prost_types::FieldMask {
                    paths: vec![
                        "object_id".into(),
                        "version".into(), 
                        "digest".into(),
                        "object_type".into(),
                        "contents".into(),
                    ],
                }),
                object_type: Some("0x2::coin::Coin<0x2::sui::SUI>".to_string()),
            })
            .await?
            .into_inner();

        for obj in resp.objects {
            if let (Some(id_str), Some(version), Some(digest_str)) = 
                (&obj.object_id, obj.version, &obj.digest) {
                    
                let object_id = sui::Address::from_str(id_str)?;
                let digest = sui::Digest::from_base58(digest_str)?;
                let object_ref = sui::ObjectReference::new(object_id, version, digest);
                
                // Extract coin balance from contents if available
                let balance = if let Some(contents) = &obj.contents {
                    if let Some(value) = &contents.value {
                        extract_coin_balance_from_contents(value)?
                    } else {
                        get_coin_balance_via_get_object(rpc_url, &object_ref).await?
                    }
                } else {
                    // Fallback: fetch object details to get balance
                    get_coin_balance_via_get_object(rpc_url, &object_ref).await?
                };
                
                if balance >= min_balance {
                    // Try to lock this coin atomically
                    if let Some(guard) = lock_manager.try_lock_coin(object_id) {
                        let coin_info = CoinInfo {
                            object_ref,
                            balance,
                        };
                        return Ok(Some((coin_info, guard)));
                    }
                }
            }
        }

        if attempt < MAX_RETRIES {
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }
    }

    println!(
        "No unlocked coins found with sufficient balance after {} attempts",
        MAX_RETRIES
    );
    Ok(None)
}

/// Extracts coin balance from BCS contents
/// Coin<T> has layout: { id: UID, balance: Balance<T> }
/// Balance<T> has layout: { value: u64 }
/// We need to skip the UID (32 bytes) and read the balance u64
fn extract_coin_balance_from_contents(contents: &[u8]) -> Result<u64> {
    if contents.len() >= 40 {
        let balance_bytes = &contents[32..40];
        let balance = u64::from_le_bytes(balance_bytes.try_into().unwrap_or([0; 8]));
        Ok(balance)
    } else {
        Ok(0)
    }
}

/// Gets the balance of a specific coin object via get_object RPC
async fn get_coin_balance_via_get_object(rpc_url: &str, object_ref: &sui::ObjectReference) -> Result<u64> {
    let mut client = GrpcClient::new(rpc_url.to_string())?;
    let mut ledger = client.ledger_client();
    
    let resp = ledger
        .get_object(proto::GetObjectRequest {
            object_id: Some(object_ref.object_id().to_string()),
            version: Some(object_ref.version()),
            read_mask: Some(prost_types::FieldMask {
                paths: vec!["contents".into()],
            }),
        })
        .await?
        .into_inner();
        
    if let Some(obj) = resp.object {
        if let Some(contents) = obj.contents {
            if let Some(value) = contents.value {
                return extract_coin_balance_from_contents(&value);
            }
        }
    }
    
    Ok(0)
}

/// Lists all available coins for a sender with their balances
pub async fn list_coins(
    rpc_url: &str,
    sender: sui::Address,
) -> Result<Vec<CoinInfo>> {
    let mut client = GrpcClient::new(rpc_url.to_string())?;
    let mut live = client.live_data_client();
    
    let resp = live
        .list_owned_objects(proto::ListOwnedObjectsRequest {
            owner: Some(sender.to_string()),
            page_size: Some(100),
            page_token: None,
            read_mask: Some(prost_types::FieldMask {
                paths: vec![
                    "object_id".into(),
                    "version".into(),
                    "digest".into(),
                    "object_type".into(),
                    "contents".into(),
                ],
            }),
            object_type: Some("0x2::coin::Coin<0x2::sui::SUI>".to_string()),
        })
        .await?
        .into_inner();

    let mut coins = Vec::new();
    
    for obj in resp.objects {
        if let (Some(id_str), Some(version), Some(digest_str)) = 
            (&obj.object_id, obj.version, &obj.digest) {
                
            let object_id = sui::Address::from_str(id_str)?;
            let digest = sui::Digest::from_base58(digest_str)?;
            let object_ref = sui::ObjectReference::new(object_id, version, digest);
            
            let balance = if let Some(contents) = &obj.contents {
                if let Some(value) = &contents.value {
                    extract_coin_balance_from_contents(value)?
                } else {
                    get_coin_balance_via_get_object(rpc_url, &object_ref).await?
                }
            } else {
                get_coin_balance_via_get_object(rpc_url, &object_ref).await?
            };
            
            coins.push(CoinInfo {
                object_ref,
                balance,
            });
        }
    }
    
    Ok(coins)
}
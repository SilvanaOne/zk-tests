//! Database module for persistent storage using RocksDB
//!
//! This module provides persistent storage for payment events, metrics, and recovery data
//! using RocksDB with multiple column families for optimized queries.

use anyhow::{Result, anyhow};
use rocksdb::{
    ColumnFamilyDescriptor, DB, DBCompressionType, Options,
    backup::{BackupEngine, BackupEngineOptions},
    checkpoint::Checkpoint, IteratorMode, Direction,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

/// Payment event stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentEvent {
    pub id: String,
    pub timestamp: SystemTime,
    pub user_party: String,
    pub user_name: String,
    pub subscription: String,
    pub amount: f64,
    pub success: bool,
    pub command_id: String,
    pub update_id: Option<String>,  // Canton update ID for tracking
    pub error_message: Option<String>,
}

/// Pending payment to be processed/retried
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPayment {
    pub id: String,
    pub user_party: String,
    pub user_name: String,
    pub subscription: String,
    pub amount: f64,
    pub scheduled_time: SystemTime,
    pub retry_count: u32,
    pub last_error: Option<String>,
}

/// Aggregated metrics for a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMetrics {
    pub window: String,
    pub timestamp: SystemTime,
    pub by_user: HashMap<String, UserMetrics>,
    pub by_subscription: HashMap<String, SubscriptionMetrics>,
    pub by_user_subscription: HashMap<(String, String), CombinedMetrics>,
}

/// User-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserMetrics {
    pub payment_count: u64,
    pub total_amount: f64,
    pub success_count: u64,
    pub failure_count: u64,
}

/// Subscription-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionMetrics {
    pub payment_count: u64,
    pub total_amount: f64,
    pub success_count: u64,
    pub failure_count: u64,
}

/// Combined user+subscription metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CombinedMetrics {
    pub payment_count: u64,
    pub total_amount: f64,
    pub success_count: u64,
    pub failure_count: u64,
}

/// RocksDB-backed payment database
#[derive(Clone)]
pub struct PaymentDatabase {
    db: Arc<DB>,
    #[allow(dead_code)]
    backup_handle: Option<Arc<JoinHandle<()>>>,
}

impl PaymentDatabase {
    #[allow(dead_code)]
    const DB_PATH: &'static str = "./payment_db";
    #[allow(dead_code)]
    const BACKUP_PATH: &'static str = "./payment_backup";
    const CF_PAYMENT_EVENTS: &'static str = "payment_events";
    const CF_PAYMENT_METRICS: &'static str = "payment_metrics";
    const CF_USER_METRICS: &'static str = "user_metrics";
    const CF_SUBSCRIPTION_METRICS: &'static str = "subscription_metrics";
    const CF_PENDING_PAYMENTS: &'static str = "pending_payments";
    const CF_LAST_PAYMENT_TIME: &'static str = "last_payment_time";

    /// Open or create the database
    pub fn open(path: &str) -> Result<Self> {
        // Configure global options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_compression_type(DBCompressionType::Lz4);

        // Configure column families
        let cf_descriptors = vec![
            // High write throughput for events
            {
                let mut event_opts = Options::default();
                event_opts.set_write_buffer_size(128 * 1024 * 1024); // 128MB
                event_opts.set_max_write_buffer_number(4);
                event_opts.set_compression_type(DBCompressionType::Lz4);
                ColumnFamilyDescriptor::new(Self::CF_PAYMENT_EVENTS, event_opts)
            },
            // Optimized for reads on metrics
            {
                let mut metrics_opts = Options::default();
                metrics_opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
                metrics_opts.optimize_for_point_lookup(1024);
                ColumnFamilyDescriptor::new(Self::CF_PAYMENT_METRICS, metrics_opts)
            },
            // User metrics
            ColumnFamilyDescriptor::new(Self::CF_USER_METRICS, Options::default()),
            // Subscription metrics
            ColumnFamilyDescriptor::new(Self::CF_SUBSCRIPTION_METRICS, Options::default()),
            // Small, frequently updated for pending payments
            {
                let mut pending_opts = Options::default();
                pending_opts.set_write_buffer_size(16 * 1024 * 1024); // 16MB
                pending_opts.set_max_write_buffer_number(2);
                ColumnFamilyDescriptor::new(Self::CF_PENDING_PAYMENTS, pending_opts)
            },
            // Last payment times
            ColumnFamilyDescriptor::new(Self::CF_LAST_PAYMENT_TIME, Options::default()),
        ];

        // Open the database
        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)?;
        let store = Self {
            db: Arc::new(db),
            backup_handle: None,
        };

        info!("📂 RocksDB database opened at {}", Self::DB_PATH);
        Ok(store)
    }

    /// Start background backup task
    #[allow(dead_code)]
    pub fn start_backup_task(&mut self) {
        let db = Arc::clone(&self.db);
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600)); // 10 minutes
            loop {
                interval.tick().await;
                if let Err(e) = Self::perform_backup(&db) {
                    error!(error = %e, "Database backup failed");
                }
            }
        });
        self.backup_handle = Some(Arc::new(handle));
    }

    /// Perform a backup
    #[allow(dead_code)]
    fn perform_backup(db: &Arc<DB>) -> Result<()> {
        let be_opts = BackupEngineOptions::new(std::path::Path::new(Self::BACKUP_PATH))?;
        std::fs::create_dir_all(Self::BACKUP_PATH)?;
        let env = rocksdb::Env::new()?;
        let mut be = BackupEngine::open(&be_opts, &env)?;
        be.create_new_backup_flush(db, true)?;
        debug!("Database backup completed");
        Ok(())
    }

    /// Create a checkpoint snapshot
    #[allow(dead_code)]
    pub fn create_snapshot(&self) -> Result<()> {
        let ts = chrono::Utc::now().format("snapshot-%Y%m%d%H%M%S");
        let target = std::path::PathBuf::from(Self::BACKUP_PATH).join(ts.to_string());
        Checkpoint::new(&self.db)?.create_checkpoint(&target)?;
        info!("Database snapshot created at {:?}", target);
        Ok(())
    }

    // Key generation functions

    /// Generate key for payment event
    fn payment_event_key(timestamp: SystemTime, user: &str, subscription: &str) -> Vec<u8> {
        let ts = timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        format!("pe:{}:{}:{}", ts, user, subscription).into_bytes()
    }

    /// Generate key for metrics
    fn metrics_key(window: &str, timestamp: SystemTime, dimension: &str, value: &str) -> Vec<u8> {
        let ts = timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        format!("m:{}:{}:{}:{}", window, ts, dimension, value).into_bytes()
    }

    /// Generate key for pending payment
    fn pending_payment_key(priority: u8, timestamp: SystemTime, id: &str) -> Vec<u8> {
        let ts = timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
        format!("pp:{}:{}:{}", priority, ts, id).into_bytes()
    }

    /// Generate key for last payment time
    fn last_payment_key(user: &str, subscription: &str) -> Vec<u8> {
        format!("lp:{}:{}", user, subscription).into_bytes()
    }

    // Database operations

    /// Record a payment event
    pub fn record_payment_event(&self, event: &PaymentEvent) -> Result<()> {
        let cf = self.db
            .cf_handle(Self::CF_PAYMENT_EVENTS)
            .ok_or_else(|| anyhow!("payment_events column family not found"))?;

        let key = Self::payment_event_key(event.timestamp, &event.user_party, &event.subscription);
        let value = bincode::serialize(event)?;
        self.db.put_cf(&cf, key, value)?;

        // Update last payment time
        self.update_last_payment_time(&event.user_party, &event.subscription, event.timestamp)?;

        debug!(
            user = %event.user_party,
            subscription = %event.subscription,
            amount = %event.amount,
            success = event.success,
            "Payment event recorded"
        );
        Ok(())
    }

    /// Update last payment time for user-subscription pair
    fn update_last_payment_time(&self, user: &str, subscription: &str, timestamp: SystemTime) -> Result<()> {
        let cf = self.db
            .cf_handle(Self::CF_LAST_PAYMENT_TIME)
            .ok_or_else(|| anyhow!("last_payment_time column family not found"))?;

        let key = Self::last_payment_key(user, subscription);
        let value = bincode::serialize(&timestamp)?;
        self.db.put_cf(&cf, key, value)?;
        Ok(())
    }

    /// Get last payment times for all user-subscription pairs
    pub fn get_last_payment_times(&self) -> Result<HashMap<(String, String), SystemTime>> {
        let cf = self.db
            .cf_handle(Self::CF_LAST_PAYMENT_TIME)
            .ok_or_else(|| anyhow!("last_payment_time column family not found"))?;

        let mut result = HashMap::new();
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            if let Some(parts) = key_str.strip_prefix("lp:") {
                let parts: Vec<&str> = parts.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let timestamp: SystemTime = bincode::deserialize(&value)?;
                    result.insert((parts[0].to_string(), parts[1].to_string()), timestamp);
                }
            }
        }

        Ok(result)
    }

    /// Add a pending payment to the queue
    pub fn add_pending_payment(&self, payment: &PendingPayment) -> Result<()> {
        let cf = self.db
            .cf_handle(Self::CF_PENDING_PAYMENTS)
            .ok_or_else(|| anyhow!("pending_payments column family not found"))?;

        // Priority: 0 = highest, higher retry count = lower priority
        let priority = payment.retry_count.min(255) as u8;
        let key = Self::pending_payment_key(priority, payment.scheduled_time, &payment.id);
        let value = bincode::serialize(payment)?;
        self.db.put_cf(&cf, key, value)?;

        info!(
            id = %payment.id,
            user = %payment.user_party,
            subscription = %payment.subscription,
            retry_count = payment.retry_count,
            "Pending payment added to queue"
        );
        Ok(())
    }

    /// Get pending payments from the queue
    pub fn get_pending_payments(&self, limit: usize) -> Result<Vec<PendingPayment>> {
        let cf = self.db
            .cf_handle(Self::CF_PENDING_PAYMENTS)
            .ok_or_else(|| anyhow!("pending_payments column family not found"))?;

        let mut payments = Vec::new();
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        for item in iter.take(limit) {
            let (_, value) = item?;
            let payment: PendingPayment = bincode::deserialize(&value)?;

            // Only include payments that are due
            if payment.scheduled_time <= SystemTime::now() {
                payments.push(payment);
            }
        }

        Ok(payments)
    }

    /// Remove a pending payment from the queue
    pub fn remove_pending_payment(&self, payment_id: &str) -> Result<()> {
        let cf = self.db
            .cf_handle(Self::CF_PENDING_PAYMENTS)
            .ok_or_else(|| anyhow!("pending_payments column family not found"))?;

        // We need to find and delete the key
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            let payment: PendingPayment = bincode::deserialize(&value)?;
            if payment.id == payment_id {
                self.db.delete_cf(&cf, key)?;
                debug!(payment_id = %payment_id, "Pending payment removed from queue");
                return Ok(());
            }
        }

        Ok(())
    }

    /// Store aggregated metrics for a time window
    pub fn store_window_metrics(&self, metrics: &WindowMetrics) -> Result<()> {
        let cf = self.db
            .cf_handle(Self::CF_PAYMENT_METRICS)
            .ok_or_else(|| anyhow!("payment_metrics column family not found"))?;

        // Store aggregated metrics
        for (user, user_metrics) in &metrics.by_user {
            let key = Self::metrics_key(&metrics.window, metrics.timestamp, "user", user);
            let value = bincode::serialize(user_metrics)?;
            self.db.put_cf(&cf, key, value)?;
        }

        for (sub, sub_metrics) in &metrics.by_subscription {
            let key = Self::metrics_key(&metrics.window, metrics.timestamp, "subscription", sub);
            let value = bincode::serialize(sub_metrics)?;
            self.db.put_cf(&cf, key, value)?;
        }

        for ((user, sub), combined) in &metrics.by_user_subscription {
            let key = Self::metrics_key(&metrics.window, metrics.timestamp, "combined", &format!("{}:{}", user, sub));
            let value = bincode::serialize(combined)?;
            self.db.put_cf(&cf, key, value)?;
        }

        debug!(window = %metrics.window, "Window metrics stored");
        Ok(())
    }

    /// Get metrics for a specific time window
    pub fn get_metrics_for_window(&self, window: &str, since: SystemTime) -> Result<WindowMetrics> {
        let cf = self.db
            .cf_handle(Self::CF_PAYMENT_METRICS)
            .ok_or_else(|| anyhow!("payment_metrics column family not found"))?;

        let mut metrics = WindowMetrics {
            window: window.to_string(),
            timestamp: SystemTime::now(),
            by_user: HashMap::new(),
            by_subscription: HashMap::new(),
            by_user_subscription: HashMap::new(),
        };

        let since_ts = since.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let prefix = format!("m:{}:", window);

        let iter = self.db.iterator_cf(&cf, IteratorMode::From(prefix.as_bytes(), Direction::Forward));
        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);

            // Check if key matches our window
            if !key_str.starts_with(&prefix) {
                break; // We've moved past our window
            }

            // Parse key: m:{window}:{timestamp}:{dimension}:{value}
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() >= 5 {
                let timestamp = parts[2].parse::<u64>().unwrap_or(0);
                if timestamp >= since_ts {
                    let dimension = parts[3];
                    let dim_value = parts[4..].join(":");

                    match dimension {
                        "user" => {
                            let user_metrics: UserMetrics = bincode::deserialize(&value)?;
                            metrics.by_user.insert(dim_value, user_metrics);
                        }
                        "subscription" => {
                            let sub_metrics: SubscriptionMetrics = bincode::deserialize(&value)?;
                            metrics.by_subscription.insert(dim_value, sub_metrics);
                        }
                        "combined" => {
                            if let Some(pos) = dim_value.find(':') {
                                let user = dim_value[..pos].to_string();
                                let sub = dim_value[pos+1..].to_string();
                                let combined: CombinedMetrics = bincode::deserialize(&value)?;
                                metrics.by_user_subscription.insert((user, sub), combined);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(metrics)
    }

    /// Get payment events in a time range
    pub fn get_payment_events(&self, since: SystemTime, until: SystemTime) -> Result<Vec<PaymentEvent>> {
        let cf = self.db
            .cf_handle(Self::CF_PAYMENT_EVENTS)
            .ok_or_else(|| anyhow!("payment_events column family not found"))?;

        let since_ts = since.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let until_ts = until.duration_since(UNIX_EPOCH).unwrap().as_secs();

        let mut events = Vec::new();
        let start_key = format!("pe:{}:", since_ts);

        let iter = self.db.iterator_cf(&cf, IteratorMode::From(start_key.as_bytes(), Direction::Forward));
        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);

            // Parse timestamp from key
            if let Some(ts_str) = key_str.strip_prefix("pe:").and_then(|s| s.split(':').next()) {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    if ts > until_ts {
                        break; // We've moved past our range
                    }
                    if ts >= since_ts {
                        let event: PaymentEvent = bincode::deserialize(&value)?;
                        events.push(event);
                    }
                }
            }
        }

        Ok(events)
    }

    /// Clean up old data
    pub fn cleanup_old_data(&self, retention_days: u32) -> Result<()> {
        let cutoff = SystemTime::now() - Duration::from_secs(retention_days as u64 * 86400);
        let cutoff_ts = cutoff.duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Clean up old payment events
        let cf = self.db
            .cf_handle(Self::CF_PAYMENT_EVENTS)
            .ok_or_else(|| anyhow!("payment_events column family not found"))?;

        let mut keys_to_delete = Vec::new();
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        for item in iter {
            let (key, _) = item?;
            let key_str = String::from_utf8_lossy(&key);

            if let Some(ts_str) = key_str.strip_prefix("pe:").and_then(|s| s.split(':').next()) {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    if ts < cutoff_ts {
                        keys_to_delete.push(key.to_vec());
                    } else {
                        break; // Keys are ordered by timestamp, so we can stop here
                    }
                }
            }
        }

        for key in keys_to_delete {
            self.db.delete_cf(&cf, &key)?;
        }

        info!(retention_days = retention_days, "Old data cleaned up");
        Ok(())
    }
}
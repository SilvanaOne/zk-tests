//! Metrics module for payment tracking and aggregation
//!
//! This module provides comprehensive payment metrics tracking with aggregation
//! across multiple time windows and persistence to RocksDB.

use crate::db::{
    CombinedMetrics, PaymentDatabase, PaymentEvent, PendingPayment,
    SubscriptionMetrics, UserMetrics, WindowMetrics,
};
use anyhow::Result;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::RwLock,
    task::JoinHandle,
    time::interval,
};
use tracing::{debug, error, info, warn};

/// Time windows for metric aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeWindow {
    TenMinutes,
    OneHour,
    SixHours,
    TwelveHours,
    OneDay,
    OneWeek,
    OneMonth,
}

impl TimeWindow {
    /// Get duration for this time window
    pub fn duration(&self) -> Duration {
        match self {
            TimeWindow::TenMinutes => Duration::from_secs(600),
            TimeWindow::OneHour => Duration::from_secs(3600),
            TimeWindow::SixHours => Duration::from_secs(21600),
            TimeWindow::TwelveHours => Duration::from_secs(43200),
            TimeWindow::OneDay => Duration::from_secs(86400),
            TimeWindow::OneWeek => Duration::from_secs(604800),
            TimeWindow::OneMonth => Duration::from_secs(2592000), // 30 days
        }
    }

    /// Get string representation for storage
    pub fn as_str(&self) -> &str {
        match self {
            TimeWindow::TenMinutes => "10m",
            TimeWindow::OneHour => "1h",
            TimeWindow::SixHours => "6h",
            TimeWindow::TwelveHours => "12h",
            TimeWindow::OneDay => "24h",
            TimeWindow::OneWeek => "7d",
            TimeWindow::OneMonth => "30d",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "10m" => Some(TimeWindow::TenMinutes),
            "1h" => Some(TimeWindow::OneHour),
            "6h" => Some(TimeWindow::SixHours),
            "12h" => Some(TimeWindow::TwelveHours),
            "24h" => Some(TimeWindow::OneDay),
            "7d" => Some(TimeWindow::OneWeek),
            "30d" => Some(TimeWindow::OneMonth),
            _ => None,
        }
    }

    /// Get all time windows
    pub fn all() -> Vec<TimeWindow> {
        vec![
            TimeWindow::TenMinutes,
            TimeWindow::OneHour,
            TimeWindow::SixHours,
            TimeWindow::TwelveHours,
            TimeWindow::OneDay,
            TimeWindow::OneWeek,
            TimeWindow::OneMonth,
        ]
    }
}

/// In-memory cache for recent metrics
#[derive(Debug, Clone)]
struct MetricsCache {
    last_update: SystemTime,
    windows: HashMap<TimeWindow, WindowMetrics>,
}

impl Default for MetricsCache {
    fn default() -> Self {
        Self {
            last_update: SystemTime::now(),
            windows: HashMap::new(),
        }
    }
}

/// Payment metrics tracker with RocksDB backend
pub struct PaymentMetrics {
    db: Arc<PaymentDatabase>,
    cache: Arc<RwLock<MetricsCache>>,
    aggregation_task: Option<JoinHandle<()>>,
}

impl PaymentMetrics {
    /// Create new payment metrics instance
    pub async fn new(db: Arc<PaymentDatabase>) -> Result<Self> {
        let metrics = Self {
            db,
            cache: Arc::new(RwLock::new(MetricsCache::default())),
            aggregation_task: None,
        };

        // Load initial metrics from database
        metrics.refresh_cache().await?;

        Ok(metrics)
    }

    /// Start background aggregation task
    pub fn start_aggregation_task(&mut self) {
        let db = Arc::clone(&self.db);
        let cache = Arc::clone(&self.cache);

        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60)); // Aggregate every minute
            loop {
                ticker.tick().await;
                if let Err(e) = Self::perform_aggregation(&db, &cache).await {
                    error!(error = %e, "Metric aggregation failed");
                }
            }
        });

        self.aggregation_task = Some(handle);
    }

    /// Perform metric aggregation
    async fn perform_aggregation(db: &Arc<PaymentDatabase>, cache: &Arc<RwLock<MetricsCache>>) -> Result<()> {
        debug!("Starting metric aggregation");

        for window in TimeWindow::all() {
            let since = SystemTime::now() - window.duration();
            let events = db.get_payment_events(since, SystemTime::now())?;

            let mut metrics = WindowMetrics {
                window: window.as_str().to_string(),
                timestamp: SystemTime::now(),
                by_user: HashMap::new(),
                by_subscription: HashMap::new(),
                by_user_subscription: HashMap::new(),
            };

            // Aggregate events
            for event in events {
                // Update user metrics
                let user_metrics = metrics.by_user
                    .entry(event.user_party.clone())
                    .or_insert_with(UserMetrics::default);

                user_metrics.payment_count += 1;
                user_metrics.total_amount += event.amount;
                if event.success {
                    user_metrics.success_count += 1;
                } else {
                    user_metrics.failure_count += 1;
                }

                // Update subscription metrics
                let sub_metrics = metrics.by_subscription
                    .entry(event.subscription.clone())
                    .or_insert_with(SubscriptionMetrics::default);

                sub_metrics.payment_count += 1;
                sub_metrics.total_amount += event.amount;
                if event.success {
                    sub_metrics.success_count += 1;
                } else {
                    sub_metrics.failure_count += 1;
                }

                // Update combined metrics
                let combined = metrics.by_user_subscription
                    .entry((event.user_party.clone(), event.subscription.clone()))
                    .or_insert_with(CombinedMetrics::default);

                combined.payment_count += 1;
                combined.total_amount += event.amount;
                if event.success {
                    combined.success_count += 1;
                } else {
                    combined.failure_count += 1;
                }
            }

            // Store in database
            db.store_window_metrics(&metrics)?;

            // Update cache
            let mut cache_guard = cache.write().await;
            cache_guard.windows.insert(window, metrics);
            cache_guard.last_update = SystemTime::now();
        }

        info!("Metric aggregation completed");
        Ok(())
    }

    /// Record a payment event
    pub async fn record_payment(&self, event: PaymentEvent) -> Result<()> {
        // Store in database
        self.db.record_payment_event(&event)?;

        // Log the event
        if event.success {
            info!(
                user = %event.user_party,
                subscription = %event.subscription,
                amount = %event.amount,
                command_id = %event.command_id,
                "Payment recorded successfully"
            );
        } else {
            warn!(
                user = %event.user_party,
                subscription = %event.subscription,
                amount = %event.amount,
                error = ?event.error_message,
                "Failed payment recorded"
            );
        }

        // Trigger cache refresh for immediate metrics update
        tokio::spawn({
            let db = Arc::clone(&self.db);
            let cache = Arc::clone(&self.cache);
            async move {
                if let Err(e) = Self::perform_aggregation(&db, &cache).await {
                    error!(error = %e, "Failed to update metrics after payment");
                }
            }
        });

        Ok(())
    }

    /// Schedule a payment for retry
    #[allow(dead_code)]
    pub async fn schedule_payment_retry(&self, payment: PendingPayment) -> Result<()> {
        self.db.add_pending_payment(&payment)?;
        info!(
            id = %payment.id,
            user = %payment.user_party,
            subscription = %payment.subscription,
            retry_count = payment.retry_count,
            "Payment scheduled for retry"
        );
        Ok(())
    }

    /// Get pending payments that are due
    #[allow(dead_code)]
    pub async fn get_due_payments(&self, limit: usize) -> Result<Vec<PendingPayment>> {
        self.db.get_pending_payments(limit)
    }

    /// Mark payment as sent
    pub async fn mark_payment_sent(&self, payment_id: &str) -> Result<()> {
        self.db.remove_pending_payment(payment_id)?;
        debug!(payment_id = %payment_id, "Payment marked as sent");
        Ok(())
    }

    /// Get metrics for a specific time window
    pub async fn get_metrics(&self, window: TimeWindow) -> Result<WindowMetrics> {
        // Check cache first
        let cache = self.cache.read().await;
        if let Some(metrics) = cache.windows.get(&window) {
            // Cache hit
            return Ok(metrics.clone());
        }
        drop(cache); // Release read lock

        // Cache miss - fetch from database
        let since = SystemTime::now() - window.duration();
        let metrics = self.db.get_metrics_for_window(window.as_str(), since)?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.windows.insert(window, metrics.clone());

        Ok(metrics)
    }

    /// Get metrics for a specific user
    pub async fn get_user_metrics(&self, user: &str, window: TimeWindow) -> Result<UserMetrics> {
        let metrics = self.get_metrics(window).await?;
        Ok(metrics.by_user
            .get(user)
            .cloned()
            .unwrap_or_default())
    }

    /// Get metrics for a specific subscription
    pub async fn get_subscription_metrics(&self, subscription: &str, window: TimeWindow) -> Result<SubscriptionMetrics> {
        let metrics = self.get_metrics(window).await?;
        Ok(metrics.by_subscription
            .get(subscription)
            .cloned()
            .unwrap_or_default())
    }

    /// Get metrics for a user-subscription combination
    pub async fn get_user_subscription_metrics(&self, user: &str, subscription: &str, window: TimeWindow) -> Result<CombinedMetrics> {
        let metrics = self.get_metrics(window).await?;
        Ok(metrics.by_user_subscription
            .get(&(user.to_string(), subscription.to_string()))
            .cloned()
            .unwrap_or_default())
    }

    /// Refresh the cache from database
    async fn refresh_cache(&self) -> Result<()> {
        let mut cache = self.cache.write().await;

        for window in TimeWindow::all() {
            let since = SystemTime::now() - window.duration();
            match self.db.get_metrics_for_window(window.as_str(), since) {
                Ok(metrics) => {
                    cache.windows.insert(window, metrics);
                }
                Err(e) => {
                    warn!(window = ?window, error = %e, "Failed to load metrics from database");
                }
            }
        }

        cache.last_update = SystemTime::now();
        Ok(())
    }

    /// Get summary statistics across all time windows
    #[allow(dead_code)]
    pub async fn get_summary_stats(&self) -> Result<SummaryStats> {
        let mut stats = SummaryStats::default();

        for window in TimeWindow::all() {
            if let Ok(metrics) = self.get_metrics(window).await {
                let window_stats = WindowStats {
                    window,
                    total_payments: metrics.by_user_subscription
                        .values()
                        .map(|m| m.payment_count)
                        .sum(),
                    total_amount: metrics.by_user_subscription
                        .values()
                        .map(|m| m.total_amount)
                        .sum(),
                    success_rate: {
                        let total_success: u64 = metrics.by_user_subscription
                            .values()
                            .map(|m| m.success_count)
                            .sum();
                        let total_failure: u64 = metrics.by_user_subscription
                            .values()
                            .map(|m| m.failure_count)
                            .sum();
                        let total = total_success + total_failure;
                        if total > 0 {
                            (total_success as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        }
                    },
                    active_users: metrics.by_user.len() as u64,
                    active_subscriptions: metrics.by_subscription.len() as u64,
                };
                stats.by_window.insert(window, window_stats);
            }
        }

        Ok(stats)
    }
}

/// Summary statistics across time windows
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SummaryStats {
    pub by_window: HashMap<TimeWindow, WindowStats>,
}

/// Statistics for a single time window
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowStats {
    pub window: TimeWindow,
    pub total_payments: u64,
    pub total_amount: f64,
    pub success_rate: f64,
    pub active_users: u64,
    pub active_subscriptions: u64,
}

/// Create a payment event from payment details
pub fn create_payment_event(
    user_party: String,
    user_name: String,
    subscription: String,
    amount: f64,
    success: bool,
    command_id: String,
    update_id: Option<String>,
    error_message: Option<String>,
) -> PaymentEvent {
    PaymentEvent {
        id: format!("{}-{}", command_id, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
        timestamp: SystemTime::now(),
        user_party,
        user_name,
        subscription,
        amount,
        success,
        command_id,
        update_id,
        error_message,
    }
}
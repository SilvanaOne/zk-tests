//! Payment recovery module for handling missed payments during downtime
//!
//! This module provides functionality to recover and retry missed payments
//! after system downtime or failures.

use crate::{
    context::ContractBlobsContext,
    db::{PaymentDatabase, PendingPayment},
    metrics::PaymentMetrics,
    pay::PaymentArgs,
    subscriptions,
    users,
};
use anyhow::Result;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Report of recovery operations
#[derive(Debug, Clone, Default)]
pub struct RecoveryReport {
    pub payments_scheduled: u64,
    pub payments_processed: u64,
    pub payments_failed: u64,
    pub total_amount: f64,
    pub errors: Vec<String>,
}

/// Payment recovery handler
pub struct PaymentRecovery {
    db: Arc<PaymentDatabase>,
    metrics: Arc<PaymentMetrics>,
    min_interval: Duration,
    max_retries: u32,
}

impl PaymentRecovery {
    /// Create new payment recovery instance
    pub fn new(db: Arc<PaymentDatabase>, metrics: Arc<PaymentMetrics>) -> Self {
        let min_interval = Duration::from_secs(
            std::env::var("PAYMENT_RETRY_INTERVAL")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30)
        );

        let max_retries = std::env::var("MAX_PAYMENT_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3);

        Self {
            db,
            metrics,
            min_interval,
            max_retries,
        }
    }

    /// Recover missed payments during downtime
    pub async fn recover_missed_payments(&self, dry_run: bool) -> Result<RecoveryReport> {
        info!("Starting missed payment recovery");
        let mut report = RecoveryReport::default();

        // Get last payment times from database
        let last_payment_times = self.db.get_last_payment_times()?;
        info!(
            total_tracked = last_payment_times.len(),
            "Retrieved last payment times"
        );

        // Get all users and subscriptions
        let users = users::get_users();
        let subscriptions = subscriptions::get_subscriptions();

        // Calculate missed payments
        let now = SystemTime::now();
        let mut missed_payments = Vec::new();

        for user in users {
            for user_sub in &user.subscriptions {
                if !user_sub.is_active() {
                    continue;
                }

                // Find subscription details
                let sub = match subscriptions.iter().find(|s| s.name == user_sub.name) {
                    Some(s) => s,
                    None => {
                        warn!(
                            user = %user.name,
                            subscription = %user_sub.name,
                            "Subscription not found in catalog"
                        );
                        continue;
                    }
                };

                // Check last payment time
                let key = (user.party.clone(), sub.name.clone());
                let last_payment = last_payment_times.get(&key).copied();

                // Calculate if payment is due
                let interval_secs = sub.interval_seconds().unwrap_or(300);
                let interval = Duration::from_secs(interval_secs);

                let should_pay = match last_payment {
                    Some(last) => {
                        let elapsed = now.duration_since(last).unwrap_or_default();
                        elapsed >= interval
                    }
                    None => true, // Never paid before
                };

                if should_pay {
                    // Calculate how many payments were missed
                    let missed_count = match last_payment {
                        Some(last) => {
                            let elapsed = now.duration_since(last).unwrap_or_default();
                            (elapsed.as_secs() / interval_secs).max(1)
                        }
                        None => 1,
                    };

                    info!(
                        user = %user.name,
                        subscription = %sub.name,
                        missed_count = missed_count,
                        "Found missed payments"
                    );

                    // Create pending payment
                    let pending = PendingPayment {
                        id: format!("recovery-{}-{}-{}", user.party, sub.name, now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()),
                        user_party: user.party.clone(),
                        user_name: user.name.clone(),
                        subscription: sub.name.clone(),
                        amount: sub.price * missed_count as f64,
                        scheduled_time: now,
                        retry_count: 0,
                        last_error: None,
                    };

                    missed_payments.push(pending);
                    report.total_amount += sub.price * missed_count as f64;
                }
            }
        }

        report.payments_scheduled = missed_payments.len() as u64;

        if dry_run {
            info!(
                payments_scheduled = report.payments_scheduled,
                total_amount = report.total_amount,
                "DRY RUN: Would schedule missed payments"
            );
        } else {
            // Schedule missed payments
            for payment in missed_payments {
                if let Err(e) = self.schedule_recovery_payment(payment).await {
                    error!(error = %e, "Failed to schedule recovery payment");
                    report.errors.push(e.to_string());
                }
            }

            info!(
                payments_scheduled = report.payments_scheduled,
                total_amount = report.total_amount,
                "Missed payments scheduled for recovery"
            );
        }

        Ok(report)
    }

    /// Schedule a recovery payment
    async fn schedule_recovery_payment(&self, payment: PendingPayment) -> Result<()> {
        self.db.add_pending_payment(&payment)?;
        debug!(
            id = %payment.id,
            user = %payment.user_party,
            subscription = %payment.subscription,
            amount = payment.amount,
            "Recovery payment scheduled"
        );
        Ok(())
    }

    /// Process pending payments queue with rate limiting
    pub async fn process_pending_queue(&self, dry_run: bool) -> Result<RecoveryReport> {
        info!("Processing pending payments queue");
        let mut report = RecoveryReport::default();

        let pending = self.db.get_pending_payments(100)?;
        info!(
            pending_count = pending.len(),
            "Retrieved pending payments"
        );

        for payment in pending {
            // Check if payment should be retried
            if payment.retry_count >= self.max_retries {
                warn!(
                    id = %payment.id,
                    user = %payment.user_party,
                    subscription = %payment.subscription,
                    retry_count = payment.retry_count,
                    "Payment exceeded max retries, skipping"
                );
                continue;
            }

            // Ensure minimum interval between payments
            sleep(self.min_interval).await;

            if dry_run {
                info!(
                    id = %payment.id,
                    user = %payment.user_party,
                    subscription = %payment.subscription,
                    amount = payment.amount,
                    "DRY RUN: Would process payment"
                );
                report.payments_processed += 1;
                report.total_amount += payment.amount;
            } else {
                match self.execute_payment(&payment).await {
                    Ok((command_id, update_id)) => {
                        info!(
                            id = %payment.id,
                            user = %payment.user_party,
                            subscription = %payment.subscription,
                            amount = payment.amount,
                            command_id = %command_id,
                            update_id = %update_id,
                            "Payment processed successfully"
                        );

                        // Remove from pending queue
                        self.metrics.mark_payment_sent(&payment.id).await?;
                        report.payments_processed += 1;
                        report.total_amount += payment.amount;
                    }
                    Err(e) => {
                        error!(
                            id = %payment.id,
                            user = %payment.user_party,
                            subscription = %payment.subscription,
                            error = %e,
                            "Payment processing failed"
                        );

                        // Handle retry
                        self.handle_payment_failure(payment, e).await?;
                        report.payments_failed += 1;
                    }
                }
            }
        }

        info!(
            processed = report.payments_processed,
            failed = report.payments_failed,
            total_amount = report.total_amount,
            "Pending queue processing complete"
        );

        Ok(report)
    }

    /// Execute a payment
    async fn execute_payment(&self, payment: &PendingPayment) -> Result<(String, String)> {
        // Load contract blobs
        let ctx = ContractBlobsContext::fetch().await?;

        // Create payment description
        let description = format!(
            "{} subscription recovery payment for {}",
            payment.subscription, payment.user_name
        );

        // Create payment args
        let payment_args = PaymentArgs::from_request(
            ctx,
            payment.amount,
            payment.user_party.clone(),
            description
        ).await?;

        // Execute payment
        let (command_id, update_id) = payment_args.execute_payment().await?;

        // Record successful payment event
        let event = crate::metrics::create_payment_event(
            payment.user_party.clone(),
            payment.user_name.clone(),
            payment.subscription.clone(),
            payment.amount,
            true,
            command_id.clone(),
            Some(update_id.clone()),
            None,
        );
        self.metrics.record_payment(event).await?;

        Ok((command_id, update_id))
    }

    /// Handle payment failure
    async fn handle_payment_failure(&self, mut payment: PendingPayment, error: anyhow::Error) -> Result<()> {
        payment.retry_count += 1;
        payment.last_error = Some(error.to_string());

        // Calculate next retry time with exponential backoff
        let backoff_secs = 60 * (2_u64.pow(payment.retry_count.min(5)));
        payment.scheduled_time = SystemTime::now() + Duration::from_secs(backoff_secs);

        if payment.retry_count < self.max_retries {
            // Reschedule for retry
            self.db.add_pending_payment(&payment)?;
            info!(
                id = %payment.id,
                retry_count = payment.retry_count,
                next_retry_secs = backoff_secs,
                "Payment rescheduled for retry"
            );
        } else {
            // Record failed payment event
            let event = crate::metrics::create_payment_event(
                payment.user_party.clone(),
                payment.user_name.clone(),
                payment.subscription.clone(),
                payment.amount,
                false,
                payment.id.clone(),
                None, // No update_id for failed payments
                payment.last_error.clone(),
            );
            self.metrics.record_payment(event).await?;

            warn!(
                id = %payment.id,
                user = %payment.user_party,
                subscription = %payment.subscription,
                "Payment failed after max retries"
            );
        }

        Ok(())
    }

    /// Calculate missed payments based on subscription intervals
    #[allow(dead_code)]
    pub async fn calculate_missed_payments(
        &self,
        last_payment_times: HashMap<(String, String), SystemTime>,
    ) -> Result<Vec<PendingPayment>> {
        let mut missed = Vec::new();
        let now = SystemTime::now();

        let users = users::get_users();
        let subscriptions = subscriptions::get_subscriptions();

        for user in users {
            for user_sub in &user.subscriptions {
                if !user_sub.is_active() {
                    continue;
                }

                let sub = match subscriptions.iter().find(|s| s.name == user_sub.name) {
                    Some(s) => s,
                    None => continue,
                };

                let key = (user.party.clone(), sub.name.clone());
                let last_payment = last_payment_times.get(&key).copied();
                let interval = Duration::from_secs(sub.interval_seconds().unwrap_or(300));

                // Calculate number of missed payments
                let missed_count = match last_payment {
                    Some(last) => {
                        let elapsed = now.duration_since(last).unwrap_or_default();
                        if elapsed >= interval {
                            (elapsed.as_secs() / interval.as_secs()).max(1)
                        } else {
                            0
                        }
                    }
                    None => 1, // Never paid, schedule one payment
                };

                for i in 0..missed_count {
                    let scheduled_time = now + Duration::from_secs(30 * i); // Space out by 30 seconds
                    missed.push(PendingPayment {
                        id: format!("missed-{}-{}-{}", user.party, sub.name, i),
                        user_party: user.party.clone(),
                        user_name: user.name.clone(),
                        subscription: sub.name.clone(),
                        amount: sub.price,
                        scheduled_time,
                        retry_count: 0,
                        last_error: None,
                    });
                }
            }
        }

        Ok(missed)
    }
}
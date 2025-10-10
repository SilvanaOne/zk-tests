//! OpenTelemetry monitoring integration for BetterStack
//!
//! This module provides OpenTelemetry support for sending logs, metrics, and traces
//! to BetterStack when the appropriate environment variables are configured.

use anyhow::{Result, anyhow};
use opentelemetry::{global, KeyValue};
use opentelemetry::logs::{LoggerProvider as _, Severity};
use opentelemetry_otlp::{LogExporter, MetricExporter, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{
    logs::LoggerProvider,
    metrics::{PeriodicReader, SdkMeterProvider},
    Resource,
};
use std::{env, sync::OnceLock, time::Duration};
use tonic::{
    metadata::MetadataMap,
    transport::{Certificate, ClientTlsConfig},
};
use tracing::{debug, info, Event, Level, Subscriber};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::{
    db::PaymentDatabase,
    metrics::{PaymentMetrics, TimeWindow},
};

/// Configuration for OpenTelemetry integration with BetterStack
#[derive(Debug, Clone)]
pub struct OpenTelemetryConfig {
    pub ingesting_host: String,
    pub source_id: String,
    pub source_token: String,
    pub canton_chain: String,
    pub app_name: String,
}

impl OpenTelemetryConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            ingesting_host: env::var("OPENTELEMETRY_INGESTING_HOST")
                .map_err(|_| anyhow!("OPENTELEMETRY_INGESTING_HOST not set"))?,
            source_id: env::var("OPENTELEMETRY_SOURCE_ID")
                .map_err(|_| anyhow!("OPENTELEMETRY_SOURCE_ID not set"))?,
            source_token: env::var("OPENTELEMETRY_SOURCE_TOKEN")
                .map_err(|_| anyhow!("OPENTELEMETRY_SOURCE_TOKEN not set"))?,
            canton_chain: env::var("CANTON_CHAIN")
                .unwrap_or_else(|_| "unknown".to_string()),
            app_name: env::var("APP_NAME")
                .unwrap_or_else(|_| "canton-billing".to_string()),
        })
    }

    /// Check if OpenTelemetry is configured
    pub fn is_configured() -> bool {
        env::var("OPENTELEMETRY_INGESTING_HOST").is_ok()
            && env::var("OPENTELEMETRY_SOURCE_ID").is_ok()
            && env::var("OPENTELEMETRY_SOURCE_TOKEN").is_ok()
    }
}

/// Global OpenTelemetry configuration
static OTEL_CONFIG: OnceLock<Option<OpenTelemetryConfig>> = OnceLock::new();

/// Global logger provider
static LOGGER_PROVIDER: OnceLock<Option<LoggerProvider>> = OnceLock::new();

/// Initialize OpenTelemetry integration
pub async fn init_opentelemetry() -> Result<()> {
    if !OpenTelemetryConfig::is_configured() {
        info!("OpenTelemetry not configured, skipping initialization");
        return Ok(());
    }

    let config = OpenTelemetryConfig::from_env()?;
    info!(
        host = %config.ingesting_host,
        source_id = %config.source_id,
        chain = %config.canton_chain,
        app = %config.app_name,
        "Initializing OpenTelemetry for BetterStack"
    );

    // Store configuration
    OTEL_CONFIG
        .set(Some(config.clone()))
        .map_err(|_| anyhow!("Failed to set OpenTelemetry configuration"))?;

    // Initialize exporters
    init_metrics_exporter(&config).await?;
    init_logs_exporter(&config).await?;

    info!("✅ OpenTelemetry initialized successfully");
    Ok(())
}

/// Initialize metrics exporter
async fn init_metrics_exporter(config: &OpenTelemetryConfig) -> Result<()> {
    let endpoint = format!("https://{}", config.ingesting_host);

    // Create metadata for authentication
    let mut metadata = MetadataMap::new();
    metadata.insert(
        "authorization",
        format!("Bearer {}", config.source_token)
            .parse()
            .map_err(|e| anyhow!("Invalid authorization header: {}", e))?,
    );

    // Build TLS config
    let tls = build_tls_config(&config.ingesting_host)?;

    // Build metric exporter
    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .with_tls_config(tls)
        .with_timeout(Duration::from_secs(10))
        .with_metadata(metadata)
        .build()
        .map_err(|e| anyhow!("Failed to build metric exporter: {}", e))?;

    // Create periodic reader
    let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_interval(Duration::from_secs(60))
        .build();

    // Create resource
    let resource = Resource::new(vec![
        KeyValue::new("service.name", config.app_name.clone()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("source.id", config.source_id.clone()),
        KeyValue::new("canton.chain", config.canton_chain.clone()),
    ]);

    // Create meter provider
    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource)
        .build();

    // Set global meter provider
    global::set_meter_provider(provider);

    debug!("Metrics exporter initialized");
    Ok(())
}

/// Initialize logs exporter
async fn init_logs_exporter(config: &OpenTelemetryConfig) -> Result<()> {
    let endpoint = format!("https://{}", config.ingesting_host);

    // Create metadata for authentication
    let mut metadata = MetadataMap::new();
    metadata.insert(
        "authorization",
        format!("Bearer {}", config.source_token)
            .parse()
            .map_err(|e| anyhow!("Invalid authorization header: {}", e))?,
    );

    // Build TLS config
    let tls = build_tls_config(&config.ingesting_host)?;

    // Build log exporter
    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .with_tls_config(tls)
        .with_timeout(Duration::from_secs(10))
        .with_metadata(metadata)
        .build()
        .map_err(|e| anyhow!("Failed to build log exporter: {}", e))?;

    // Create resource
    let resource = Resource::new(vec![
        KeyValue::new("service.name", config.app_name.clone()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("source.id", config.source_id.clone()),
        KeyValue::new("canton.chain", config.canton_chain.clone()),
    ]);

    // Create logger provider
    let provider = LoggerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(resource)
        .build();

    // Store provider
    LOGGER_PROVIDER
        .set(Some(provider))
        .map_err(|_| anyhow!("Failed to set logger provider"))?;

    debug!("Logs exporter initialized");
    Ok(())
}

/// Build TLS configuration
fn build_tls_config(host: &str) -> Result<ClientTlsConfig> {
    // Use ISRG Root X1 certificate
    let pem_bytes = include_bytes!("./isrg-root-x1.pem");
    let ca = Certificate::from_pem(pem_bytes);

    Ok(ClientTlsConfig::new()
        .ca_certificate(ca)
        .domain_name(host.to_owned()))
}

/// Custom tracing layer for BetterStack
pub struct BetterStackLayer;

impl<S> Layer<S> for BetterStackLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Only forward warn and error level logs
        let level = *event.metadata().level();

        // Skip logs from monitoring module to prevent recursion
        let target = event.metadata().target();
        if target.starts_with("monitoring::") || target.contains("opentelemetry") {
            return;
        }

        if level <= Level::WARN {
            // Extract message
            let mut visitor = LogVisitor::default();
            event.record(&mut visitor);

            if let Some(message) = visitor.get_message() {
                let severity = match level {
                    Level::ERROR => Severity::Error,
                    Level::WARN => Severity::Warn,
                    Level::INFO => Severity::Info,
                    Level::DEBUG => Severity::Debug,
                    Level::TRACE => Severity::Trace,
                };

                // Send to BetterStack asynchronously
                tokio::spawn(async move {
                    send_log_to_betterstack(severity, &message).await;
                });
            }
        }
    }
}

/// Log visitor for extracting messages
#[derive(Default)]
struct LogVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let field_str = format!("{}={:?}", field.name(), value);
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value).trim_matches('"').to_string());
        } else {
            self.fields.push(field_str);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }
}

impl LogVisitor {
    fn get_message(&self) -> Option<String> {
        match &self.message {
            Some(msg) => {
                if self.fields.is_empty() {
                    Some(msg.clone())
                } else {
                    Some(format!("{} ({})", msg, self.fields.join(", ")))
                }
            }
            None => {
                if !self.fields.is_empty() {
                    Some(self.fields.join(", "))
                } else {
                    None
                }
            }
        }
    }
}

/// Send log to BetterStack
async fn send_log_to_betterstack(severity: Severity, message: &str) {
    if let Some(Some(provider)) = LOGGER_PROVIDER.get() {
        use opentelemetry::logs::{Logger, AnyValue, LogRecord as _};

        let logger = provider.logger("canton-billing");

        let mut record = logger.create_log_record();
        record.set_severity_number(severity);
        record.set_severity_text(match severity {
            Severity::Error | Severity::Error2 | Severity::Error3 | Severity::Error4 => "ERROR",
            Severity::Warn | Severity::Warn2 | Severity::Warn3 | Severity::Warn4 => "WARN",
            Severity::Info | Severity::Info2 | Severity::Info3 | Severity::Info4 => "INFO",
            Severity::Debug | Severity::Debug2 | Severity::Debug3 | Severity::Debug4 => "DEBUG",
            _ => "TRACE",
        });
        record.set_body(AnyValue::from(message.to_string()));

        logger.emit(record);
    }
}

/// Initialize logging with BetterStack integration
pub async fn init_logging() -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info".into());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_ansi(true)
        .compact();

    if OpenTelemetryConfig::is_configured() {
        // With BetterStack integration
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(BetterStackLayer)
            .init();
        info!("📝 Logging initialized with BetterStack integration");
    } else {
        // Without BetterStack integration
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        info!("📝 Logging initialized (local only)");
    }

    Ok(())
}

/// Export payment metrics to OpenTelemetry
pub async fn export_metrics_to_opentelemetry(
    _db: &PaymentDatabase,
    metrics: &PaymentMetrics,
) -> Result<()> {
    if !OpenTelemetryConfig::is_configured() {
        return Ok(());
    }

    let meter = global::meter("canton-billing");

    // Export metrics for each time window
    for window in TimeWindow::all() {
        let window_metrics = metrics.get_metrics(window).await?;
        let window_str = window.as_str();

        // Payment count metric
        let count_gauge = meter
            .u64_gauge(format!("canton.billing.window.{}.payment_count", window_str))
            .with_description(format!("Payment count for {} window", window_str))
            .build();

        // Total amount metric
        let amount_gauge = meter
            .f64_gauge(format!("canton.billing.window.{}.total_amount", window_str))
            .with_description(format!("Total amount for {} window", window_str))
            .build();

        // Success rate metric
        let success_gauge = meter
            .f64_gauge(format!("canton.billing.window.{}.success_rate", window_str))
            .with_description(format!("Success rate for {} window", window_str))
            .build();

        // Export per user-subscription metrics
        for ((user, subscription), combined) in &window_metrics.by_user_subscription {
            let attributes = vec![
                KeyValue::new("user", user.clone()),
                KeyValue::new("subscription", subscription.clone()),
                KeyValue::new("window", window_str.to_string()),
            ];

            count_gauge.record(combined.payment_count, &attributes);
            amount_gauge.record(combined.total_amount, &attributes);

            let success_rate = if combined.payment_count > 0 {
                (combined.success_count as f64 / combined.payment_count as f64) * 100.0
            } else {
                0.0
            };
            success_gauge.record(success_rate, &attributes);

            // Add failure count metric
            let failure_gauge = meter
                .u64_gauge(format!("canton.billing.window.{}.failure_count", window_str))
                .with_description(format!("Number of failed payments for {} window", window_str))
                .build();
            failure_gauge.record(combined.failure_count, &attributes);
        }

        // Calculate overall metrics
        let total_payment_count: u64 = window_metrics
            .by_user_subscription
            .values()
            .map(|m| m.payment_count)
            .sum();

        let total_amount: f64 = window_metrics
            .by_user_subscription
            .values()
            .map(|m| m.total_amount)
            .sum();

        let total_success: u64 = window_metrics
            .by_user_subscription
            .values()
            .map(|m| m.success_count)
            .sum();

        let total_failure: u64 = window_metrics
            .by_user_subscription
            .values()
            .map(|m| m.failure_count)
            .sum();

        // Export overall metrics (without user/subscription attributes)
        let overall_attributes = vec![KeyValue::new("window", window_str.to_string())];

        count_gauge.record(total_payment_count, &overall_attributes);
        amount_gauge.record(total_amount, &overall_attributes);

        let overall_success_rate = if total_payment_count > 0 {
            (total_success as f64 / total_payment_count as f64) * 100.0
        } else {
            0.0
        };
        success_gauge.record(overall_success_rate, &overall_attributes);

        // Add overall failure count
        let failure_gauge = meter
            .u64_gauge(format!("canton.billing.window.{}.failure_count", window_str))
            .with_description(format!("Number of failed payments for {} window", window_str))
            .build();
        failure_gauge.record(total_failure, &overall_attributes);

        // Active users metric
        let active_users_gauge = meter
            .u64_gauge(format!("canton.billing.window.{}.active_users", window_str))
            .with_description(format!("Active users for {} window", window_str))
            .build();

        active_users_gauge.record(
            window_metrics.by_user.len() as u64,
            &overall_attributes,
        );

        // Active subscriptions metric
        let active_subs_gauge = meter
            .u64_gauge(format!("canton.billing.window.{}.active_subscriptions", window_str))
            .with_description(format!("Active subscriptions for {} window", window_str))
            .build();

        active_subs_gauge.record(
            window_metrics.by_subscription.len() as u64,
            &overall_attributes,
        );
    }

    debug!("Metrics exported to OpenTelemetry");
    Ok(())
}

/// Export a failed payment event to OpenTelemetry
pub async fn export_failed_payment_event(
    user: &str,
    subscription: &str,
    amount: f64,
    error: &str,
) {
    if !OpenTelemetryConfig::is_configured() {
        return;
    }

    let meter = global::meter("canton-billing");

    // Create a gauge for failed payment events - this allows BetterStack to capture the error details
    let failed_payment_gauge = meter
        .f64_gauge("canton.billing.payment.failed")
        .with_description("Failed payment event")
        .build();

    failed_payment_gauge.record(
        amount,
        &[
            KeyValue::new("user", user.to_string()),
            KeyValue::new("subscription", subscription.to_string()),
            KeyValue::new("error", error.to_string()),
            KeyValue::new("timestamp", chrono::Utc::now().to_rfc3339()),
        ],
    );

    debug!(
        user = %user,
        subscription = %subscription,
        amount = amount,
        error = %error,
        "Failed payment event exported to OpenTelemetry"
    );
}

/// Test OpenTelemetry integration
#[allow(dead_code)]
pub async fn test_integration() -> Result<()> {
    if !OpenTelemetryConfig::is_configured() {
        return Ok(());
    }

    info!("Testing OpenTelemetry integration with BetterStack");

    // Test metric
    let meter = global::meter("canton-billing");
    let test_gauge = meter
        .f64_gauge("canton.billing.test.metric")
        .with_description("Test metric for integration verification")
        .build();

    test_gauge.record(
        42.0,
        &[
            KeyValue::new("test", "true"),
            KeyValue::new("timestamp", chrono::Utc::now().to_rfc3339()),
        ],
    );

    // Test log
    send_log_to_betterstack(
        Severity::Info,
        "OpenTelemetry integration test successful",
    ).await;

    info!("✅ OpenTelemetry test completed");
    Ok(())
}
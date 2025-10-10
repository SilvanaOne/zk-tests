//! Subscription management module that loads subscription data from JSON file

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::OnceLock;
use tracing::{info, error, warn};

/// Global subscription list loaded once on first access
static SUBSCRIPTIONS: OnceLock<Vec<Subscription>> = OnceLock::new();

/// Represents a single subscription plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    /// Unique identifier for the subscription
    pub id: String,

    /// Name of the subscription plan
    pub name: String,

    /// Description of the subscription
    pub description: Option<String>,

    /// Price of the subscription
    pub price: f64,

    /// Currency for the price (e.g., "CC")
    pub currency: String,

    /// Interval in seconds as a string (to match JSON format)
    pub interval_sec: String,
}

/// Root structure for the subscriptions JSON file
#[derive(Debug, Deserialize)]
struct SubscriptionsFile {
    subscriptions: Vec<Subscription>,
}

impl Subscription {
    /// Get the interval as seconds (parsed from string)
    pub fn interval_seconds(&self) -> Result<u64, std::num::ParseIntError> {
        self.interval_sec.parse()
    }

    /// Get a formatted price string with currency
    pub fn formatted_price(&self) -> String {
        format!("{} {}", self.price, self.currency)
    }

    /// Validate that this subscription meets all requirements
    pub fn validate(&self) -> anyhow::Result<()> {
        // Currency must be CC
        if self.currency != "CC" {
            return Err(anyhow::anyhow!(
                "Invalid currency for subscription '{}': expected 'CC', got '{}'",
                self.name,
                self.currency
            ));
        }

        // Interval must be parseable and greater than 60
        let interval = self.interval_seconds().map_err(|e| {
            anyhow::anyhow!(
                "Invalid interval_sec for subscription '{}': {}",
                self.name,
                e
            )
        })?;

        if interval <= 60 {
            return Err(anyhow::anyhow!(
                "Invalid interval_sec for subscription '{}': must be greater than 60 seconds, got {}",
                self.name,
                interval
            ));
        }

        Ok(())
    }
}

/// Load subscriptions from the JSON file (called once via OnceLock)
fn load_subscriptions_from_file() -> anyhow::Result<Vec<Subscription>> {
    let file_path = "subscriptions.json";

    // Read the file
    let contents = fs::read_to_string(file_path)
        .map_err(|e| anyhow::anyhow!("Failed to read subscriptions.json: {}", e))?;

    // Parse the JSON
    let data: SubscriptionsFile = serde_json::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse subscriptions.json: {}", e))?;

    // Validate each subscription
    for sub in &data.subscriptions {
        sub.validate()?;
    }

    Ok(data.subscriptions)
}

/// Get the list of available subscriptions
/// Loads from file on first call, then returns cached version
pub fn get_subscriptions() -> &'static Vec<Subscription> {
    SUBSCRIPTIONS.get_or_init(|| match load_subscriptions_from_file() {
        Ok(subs) => {
            info!(count = subs.len(), "Loaded subscriptions from subscriptions.json");
            subs
        }
        Err(e) => {
            error!(error = %e, "Error loading subscriptions");
            warn!("Using empty subscription list");
            Vec::new()
        }
    })
}

/// Find a subscription by ID
#[allow(dead_code)]
pub fn find_subscription_by_id(id: &str) -> Option<&'static Subscription> {
    get_subscriptions().iter().find(|s| s.id == id)
}

/// Find a subscription by name
pub fn find_subscription_by_name(name: &str) -> Option<&'static Subscription> {
    get_subscriptions().iter().find(|s| s.name == name)
}

/// List all available subscriptions with formatted output
pub fn list_subscriptions() {
    let subs = get_subscriptions();

    if subs.is_empty() {
        println!("No subscriptions available");
        return;
    }

    println!("\n📋 Available Subscriptions ({} total):", subs.len());
    println!("{:-<60}", "");

    for sub in subs {
        println!("\n🔸 {} ({})", sub.name, sub.id);
        if let Some(desc) = &sub.description {
            println!("   Description: {}", desc);
        }
        println!("   Price: {}", sub.formatted_price());
        println!("   Billing Interval: {} seconds", sub.interval_sec);
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_interval_parsing() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: Some("Test subscription".to_string()),
            price: 1.0,
            currency: "CC".to_string(),
            interval_sec: "120".to_string(),
        };

        assert_eq!(sub.interval_seconds().unwrap(), 120);
    }

    #[test]
    fn test_formatted_price() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: None,
            price: 2.5,
            currency: "CC".to_string(),
            interval_sec: "120".to_string(),
        };

        assert_eq!(sub.formatted_price(), "2.5 CC");
    }

    #[test]
    fn test_validate_valid_subscription() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: Some("Test subscription".to_string()),
            price: 1.0,
            currency: "CC".to_string(),
            interval_sec: "120".to_string(),
        };

        assert!(sub.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_currency() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: Some("Test subscription".to_string()),
            price: 1.0,
            currency: "USD".to_string(),
            interval_sec: "120".to_string(),
        };

        let result = sub.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid currency"));
    }

    #[test]
    fn test_validate_interval_too_small() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: Some("Test subscription".to_string()),
            price: 1.0,
            currency: "CC".to_string(),
            interval_sec: "60".to_string(),
        };

        let result = sub.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must be greater than 60")
        );
    }

    #[test]
    fn test_validate_interval_exactly_61() {
        let sub = Subscription {
            id: "1".to_string(),
            name: "test".to_string(),
            description: Some("Test subscription".to_string()),
            price: 1.0,
            currency: "CC".to_string(),
            interval_sec: "61".to_string(),
        };

        assert!(sub.validate().is_ok());
    }
}

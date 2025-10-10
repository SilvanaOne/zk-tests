//! User management module that loads user data from JSON file

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::fs;
use chrono::NaiveDate;
use tracing::{debug, info, error, warn};

/// Global user list loaded once on first access
static USERS: OnceLock<Vec<User>> = OnceLock::new();

/// Represents a user's subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSubscription {
    /// Name of the subscription (should match subscription names in subscriptions.json)
    pub name: String,

    /// Expiration date in YYYY-MM-DD format
    pub expires_at: String,
}

/// Represents a user in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Unique identifier for the user
    pub id: String,

    /// Canton party identifier
    pub party: String,

    /// User's display name
    pub name: String,

    /// User's email address
    pub email: String,

    /// List of user's active subscriptions
    pub subscriptions: Vec<UserSubscription>,
}

/// Root structure for the users JSON file
#[derive(Debug, Deserialize)]
struct UsersFile {
    users: Vec<User>,
}

impl UserSubscription {
    /// Parse the expiration date
    pub fn expiration_date(&self) -> Result<NaiveDate, chrono::ParseError> {
        NaiveDate::parse_from_str(&self.expires_at, "%Y-%m-%d")
    }

    /// Check if the subscription is expired (based on today's date)
    pub fn is_expired(&self) -> bool {
        match self.expiration_date() {
            Ok(expiry) => {
                let today = chrono::Local::now().date_naive();
                expiry < today
            }
            Err(_) => true, // Treat invalid dates as expired
        }
    }

    /// Check if the subscription is active (not expired)
    pub fn is_active(&self) -> bool {
        !self.is_expired()
    }

    /// Validate that this user subscription references a valid subscription
    pub fn validate(&self) -> anyhow::Result<()> {
        // Check if subscription exists in subscriptions list
        let subscriptions = crate::subscriptions::get_subscriptions();
        let found = subscriptions.iter().any(|s| s.name == self.name);

        if !found {
            return Err(anyhow::anyhow!(
                "User subscription references unknown subscription: '{}'",
                self.name
            ));
        }

        // Validate date format
        self.expiration_date()
            .map_err(|e| anyhow::anyhow!(
                "Invalid expiration date '{}' for subscription '{}': {}",
                self.expires_at,
                self.name,
                e
            ))?;

        Ok(())
    }
}

impl User {
    /// Get all active (non-expired) subscriptions for this user
    pub fn active_subscriptions(&self) -> Vec<&UserSubscription> {
        self.subscriptions
            .iter()
            .filter(|s| s.is_active())
            .collect()
    }

    /// Get all expired subscriptions for this user
    pub fn expired_subscriptions(&self) -> Vec<&UserSubscription> {
        self.subscriptions
            .iter()
            .filter(|s| s.is_expired())
            .collect()
    }

    /// Check if user has a specific active subscription
    pub fn has_active_subscription(&self, subscription_name: &str) -> bool {
        self.subscriptions
            .iter()
            .any(|s| s.name == subscription_name && s.is_active())
    }

    /// Validate that this user's data is correct
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate party format (should contain ::)
        if !self.party.contains("::") {
            return Err(anyhow::anyhow!(
                "Invalid party format for user '{}': expected format 'name::hash'",
                self.name
            ));
        }

        // Validate email format (basic check)
        if !self.email.contains('@') {
            return Err(anyhow::anyhow!(
                "Invalid email format for user '{}': '{}'",
                self.name,
                self.email
            ));
        }

        // Validate each subscription
        for sub in &self.subscriptions {
            sub.validate()?;
        }

        Ok(())
    }

    /// Get a formatted string of user's subscriptions
    pub fn subscriptions_summary(&self) -> String {
        if self.subscriptions.is_empty() {
            return "No subscriptions".to_string();
        }

        let active = self.active_subscriptions();
        let expired = self.expired_subscriptions();

        let mut summary = Vec::new();

        if !active.is_empty() {
            let active_names: Vec<String> = active.iter()
                .map(|s| format!("{} (expires {})", s.name, s.expires_at))
                .collect();
            summary.push(format!("Active: {}", active_names.join(", ")));
        }

        if !expired.is_empty() {
            let expired_names: Vec<String> = expired.iter()
                .map(|s| format!("{} (expired {})", s.name, s.expires_at))
                .collect();
            summary.push(format!("Expired: {}", expired_names.join(", ")));
        }

        summary.join(" | ")
    }
}

/// Load users from the JSON file (called once via OnceLock)
fn load_users_from_file() -> anyhow::Result<Vec<User>> {
    let file_path = "users.json";

    // Read the file
    let contents = fs::read_to_string(file_path)
        .map_err(|e| anyhow::anyhow!("Failed to read users.json: {}", e))?;

    // Parse the JSON
    let data: UsersFile = serde_json::from_str(&contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse users.json: {}", e))?;

    // Validate each user
    for user in &data.users {
        user.validate()?;
    }

    Ok(data.users)
}

/// Get the list of all users
/// Loads from file on first call, then returns cached version
pub fn get_users() -> &'static Vec<User> {
    USERS.get_or_init(|| {
        match load_users_from_file() {
            Ok(users) => {
                info!(count = users.len(), "Loaded users from users.json");
                users
            }
            Err(e) => {
                error!(error = %e, "Error loading users");
                warn!("Using empty user list");
                Vec::new()
            }
        }
    })
}

/// Find a user by ID
pub fn find_user_by_id(id: &str) -> Option<&'static User> {
    get_users().iter().find(|u| u.id == id)
}

/// Find a user by party identifier
pub fn find_user_by_party(party: &str) -> Option<&'static User> {
    get_users().iter().find(|u| u.party == party)
}

/// Find a user by email
pub fn find_user_by_email(email: &str) -> Option<&'static User> {
    get_users().iter().find(|u| u.email == email)
}

/// Find all users with a specific subscription
pub fn find_users_with_subscription(subscription_name: &str) -> Vec<&'static User> {
    get_users()
        .iter()
        .filter(|u| u.has_active_subscription(subscription_name))
        .collect()
}

/// List all users with formatted output
pub fn list_users() {
    let users = get_users();

    if users.is_empty() {
        warn!("No users available");
        return;
    }

    info!(count = users.len(), "Registered users");

    for user in users {
        info!(
            id = %user.id,
            name = %user.name,
            email = %user.email,
            party = %user.party,
            subscriptions = %user.subscriptions_summary(),
            "User details"
        );
    }
}

/// List users with a specific subscription
pub fn list_users_with_subscription(subscription_name: &str) {
    let users = find_users_with_subscription(subscription_name);

    if users.is_empty() {
        warn!(subscription = %subscription_name, "No users have this subscription");
        return;
    }

    info!(
        subscription = %subscription_name,
        count = users.len(),
        "Users with subscription"
    );

    for user in users {
        // Find the specific subscription details
        let expires = user.subscriptions.iter()
            .find(|sub| sub.name == subscription_name && sub.is_active())
            .map(|sub| sub.expires_at.as_str())
            .unwrap_or("N/A");

        info!(
            name = %user.name,
            email = %user.email,
            expires = %expires,
            "User with subscription"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_subscription(name: &str, expires: &str) -> UserSubscription {
        UserSubscription {
            name: name.to_string(),
            expires_at: expires.to_string(),
        }
    }

    fn create_test_user() -> User {
        User {
            id: "1".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            party: "testparty::12345".to_string(),
            subscriptions: vec![
                create_test_subscription("prover", "2026-01-01"),
                create_test_subscription("verifier", "2020-01-01"), // expired
            ],
        }
    }

    #[test]
    fn test_subscription_expiration() {
        let future_sub = create_test_subscription("test", "2030-01-01");
        assert!(!future_sub.is_expired());
        assert!(future_sub.is_active());

        let past_sub = create_test_subscription("test", "2020-01-01");
        assert!(past_sub.is_expired());
        assert!(!past_sub.is_active());
    }

    #[test]
    fn test_user_active_subscriptions() {
        let user = create_test_user();
        let active = user.active_subscriptions();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "prover");
    }

    #[test]
    fn test_user_expired_subscriptions() {
        let user = create_test_user();
        let expired = user.expired_subscriptions();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].name, "verifier");
    }

    #[test]
    fn test_has_active_subscription() {
        let user = create_test_user();
        assert!(user.has_active_subscription("prover"));
        assert!(!user.has_active_subscription("verifier")); // expired
        assert!(!user.has_active_subscription("nonexistent"));
    }

    #[test]
    fn test_validate_invalid_party() {
        let mut user = create_test_user();
        user.party = "invalidparty".to_string(); // no ::
        assert!(user.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_email() {
        let mut user = create_test_user();
        user.email = "invalidemail".to_string(); // no @
        assert!(user.validate().is_err());
    }

    #[test]
    fn test_subscriptions_summary() {
        let user = create_test_user();
        let summary = user.subscriptions_summary();
        assert!(summary.contains("Active:"));
        assert!(summary.contains("Expired:"));
        assert!(summary.contains("prover"));
        assert!(summary.contains("verifier"));
    }
}
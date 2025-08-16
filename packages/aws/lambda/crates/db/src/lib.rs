use std::sync::{Arc, OnceLock};

// Module declarations
pub mod lock;
pub mod secure_storage;

// Re-export commonly used types
pub use lock::{KeyLock, LockGuard};

// Global static to store the DynamoDB client configuration (shared across modules)
static DYNAMODB_CLIENT: OnceLock<Arc<aws_sdk_dynamodb::Client>> = OnceLock::new();
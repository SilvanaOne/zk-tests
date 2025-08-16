// Module declarations
pub mod add;
pub mod chain;
pub mod client;
pub mod keypair;
pub mod registry;

// Re-export commonly used types
pub use client::SuiClient;
pub use registry::{create_registry, CreateRegistryResult};
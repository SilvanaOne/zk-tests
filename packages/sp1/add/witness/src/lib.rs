// Public modules - expose all functionality
pub mod binance;
pub mod price_proof;
pub mod sui;
pub mod tsa;

// Re-export commonly used functions for convenience
pub use binance::fetch_and_verify_price;
pub use price_proof::fetch_price_proof_data;
pub use sui::get_last_checkpoint;
pub use tsa::get_timestamp;

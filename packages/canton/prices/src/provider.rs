use anyhow::Result;
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// Common ticker data structure across all providers
#[derive(Debug, Clone)]
pub struct TickerData {
    pub symbol: String,
    pub last_price: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub high_price: String,
    pub low_price: String,
    pub volume: String,
    pub quote_volume: String,
    pub bid_price: String,
    pub bid_qty: String,
    pub ask_price: String,
    pub ask_qty: String,
    pub open_price: String,
    pub prev_close_price: String,
    pub open_time: i64,
    pub close_time: i64,
    pub count: i64,
}

/// Trait for provider WebSocket streaming
#[async_trait]
pub trait ProviderWebSocket: Send + Sync {
    /// Create a new WebSocket client for the given symbols
    #[allow(dead_code)]
    fn new(symbols: Vec<String>) -> Self
    where
        Self: Sized;

    /// Connect to the WebSocket with automatic retry
    async fn connect_with_retry(
        &mut self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>>;

    /// Get the provider name
    #[allow(dead_code)]
    fn provider_name(&self) -> &str;
}

/// Trait for provider REST API
#[async_trait]
pub trait ProviderRest: Send + Sync {
    /// Create a new REST client
    fn new() -> Self
    where
        Self: Sized;

    /// Fetch 24hr ticker data for the given symbols
    async fn get_24hr_ticker(&self, symbols: &[String]) -> Result<Vec<TickerData>>;

    /// Get the provider name
    #[allow(dead_code)]
    fn provider_name(&self) -> &str;
}

/// Provider type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Binance,
    Bybit,
    Coinbase,
}

impl ProviderType {
    /// Parse provider type from string
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "binance" => Ok(ProviderType::Binance),
            "bybit" => Ok(ProviderType::Bybit),
            "coinbase" => Ok(ProviderType::Coinbase),
            _ => Err(format!(
                "Unknown provider: {}. Supported providers: binance, bybit, coinbase",
                s
            )),
        }
    }

    /// Get provider name as string
    pub fn as_str(&self) -> &str {
        match self {
            ProviderType::Binance => "binance",
            ProviderType::Bybit => "bybit",
            ProviderType::Coinbase => "coinbase",
        }
    }
}

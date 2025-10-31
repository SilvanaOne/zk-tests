use serde::{Deserialize, Serialize};
use serde_json::Value;

/// WebSocket message wrapper
#[derive(Debug, Deserialize, Clone)]
pub struct BybitWsMessage {
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    pub msg_type: Option<String>,  // "snapshot" or "delta"
    #[serde(default)]
    #[allow(dead_code)]
    pub ts: Option<u64>,  // Timestamp when data generated
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cts: Option<u64>,  // Matching engine timestamp
    // For subscription responses
    #[serde(default)]
    pub op: Option<String>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub ret_msg: Option<String>,
    #[serde(default)]
    pub conn_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub req_id: Option<String>,
}

/// Trade data from publicTrade topic
#[derive(Debug, Deserialize, Clone)]
pub struct TradeData {
    #[serde(rename = "T")]
    pub timestamp: u64,  // Trade timestamp in ms
    #[serde(rename = "s")]
    pub symbol: String,  // Symbol name
    #[serde(rename = "S")]
    pub side: String,  // "Buy" or "Sell"
    #[serde(rename = "v")]
    pub volume: String,  // Trade size
    #[serde(rename = "p")]
    pub price: String,  // Trade price
    #[serde(rename = "i")]
    pub trade_id: String,  // Trade ID
    #[serde(rename = "BT", default)]
    #[allow(dead_code)]
    pub block_trade: Option<bool>,  // Whether it's a block trade
}

/// Ticker data from tickers topic
#[derive(Debug, Deserialize, Clone)]
pub struct TickerData {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "price24hPcnt")]
    pub price_24h_pcnt: String,  // Price change percentage
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    #[serde(rename = "turnover24h")]
    #[allow(dead_code)]
    pub turnover_24h: String,
    #[serde(rename = "highPrice24h")]
    pub high_price_24h: String,
    #[serde(rename = "lowPrice24h")]
    pub low_price_24h: String,
    #[serde(rename = "prevPrice24h", default)]
    #[allow(dead_code)]
    pub prev_price_24h: Option<String>,
    #[serde(rename = "bid1Price", default)]
    #[allow(dead_code)]
    pub bid1_price: Option<String>,
    #[serde(rename = "ask1Price", default)]
    #[allow(dead_code)]
    pub ask1_price: Option<String>,
    #[serde(rename = "bid1Size", default)]
    #[allow(dead_code)]
    pub bid1_size: Option<String>,
    #[serde(rename = "ask1Size", default)]
    #[allow(dead_code)]
    pub ask1_size: Option<String>,
}

/// Subscription request
#[derive(Debug, Serialize, Clone)]
pub struct SubscriptionRequest {
    pub op: String,  // "subscribe" or "unsubscribe"
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_id: Option<String>,
}

/// Ping message
#[derive(Debug, Serialize, Clone)]
pub struct PingMessage {
    pub op: String,  // "ping"
}

/// Enum for different message types
#[derive(Debug, Clone)]
pub enum BybitMessage {
    Trade(Vec<TradeData>),
    Ticker(TickerData),
    Subscription(SubscriptionResponse),
    Pong,
    Unknown(Value),
}

/// Subscription response
#[derive(Debug, Deserialize, Clone)]
pub struct SubscriptionResponse {
    #[allow(dead_code)]
    pub success: bool,
    #[allow(dead_code)]
    pub op: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub ret_msg: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub conn_id: Option<String>,
}

/// REST API ticker response wrapper
#[derive(Debug, Deserialize, Clone)]
pub struct TickersResponse {
    #[serde(rename = "retCode")]
    pub ret_code: i32,
    #[serde(rename = "retMsg")]
    pub ret_msg: String,
    pub result: TickersResult,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TickersResult {
    #[allow(dead_code)]
    pub category: String,
    pub list: Vec<TickerRestData>,
}

/// REST API ticker data
#[derive(Debug, Deserialize, Clone)]
pub struct TickerRestData {
    pub symbol: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "highPrice24h")]
    pub high_price_24h: String,
    #[serde(rename = "lowPrice24h")]
    pub low_price_24h: String,
    #[serde(rename = "volume24h")]
    pub volume_24h: String,
    #[serde(rename = "turnover24h")]
    pub turnover_24h: String,
    #[serde(rename = "price24hPcnt")]
    pub price_24h_pcnt: String,
    #[serde(rename = "prevPrice24h", default)]
    pub prev_price_24h: Option<String>,
    #[serde(rename = "bid1Price")]
    pub bid1_price: String,
    #[serde(rename = "ask1Price")]
    pub ask1_price: String,
    #[serde(rename = "bid1Size")]
    pub bid1_size: String,
    #[serde(rename = "ask1Size")]
    pub ask1_size: String,
    #[serde(rename = "openInterest", default)]
    #[allow(dead_code)]
    pub open_interest: Option<String>,
    #[serde(rename = "usdIndexPrice", default)]
    #[allow(dead_code)]
    pub usd_index_price: Option<String>,
}

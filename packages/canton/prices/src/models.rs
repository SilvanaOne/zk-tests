use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wrapper for stream messages when using combined streams
#[derive(Debug, Deserialize)]
pub struct StreamMessage {
    #[allow(dead_code)]
    pub stream: String,
    #[allow(dead_code)]
    pub data: Value,
}

/// Trade event from individual symbol trade stream
#[derive(Debug, Deserialize, Clone)]
pub struct TradeEvent {
    #[serde(rename = "e")]
    #[allow(dead_code)]
    pub event_type: String,      // "trade"

    #[serde(rename = "E")]
    #[allow(dead_code)]
    pub event_time: u64,          // Event time

    #[serde(rename = "s")]
    #[allow(dead_code)]
    pub symbol: String,           // Symbol

    #[serde(rename = "t")]
    #[allow(dead_code)]
    pub trade_id: u64,            // Trade ID

    #[serde(rename = "p")]
    #[allow(dead_code)]
    pub price: String,            // Price

    #[serde(rename = "q")]
    #[allow(dead_code)]
    pub quantity: String,         // Quantity

    #[serde(rename = "b", default)]
    #[allow(dead_code)]
    pub buyer_order_id: Option<u64>,      // Buyer order ID (optional)

    #[serde(rename = "a", default)]
    #[allow(dead_code)]
    pub seller_order_id: Option<u64>,     // Seller order ID (optional)

    #[serde(rename = "T")]
    #[allow(dead_code)]
    pub trade_time: u64,          // Trade time

    #[serde(rename = "m")]
    #[allow(dead_code)]
    pub is_buyer_maker: bool,     // Is buyer the market maker?

    #[serde(rename = "M", default)]
    #[allow(dead_code)]
    pub is_best_match: Option<bool>,     // Is best price match (optional)
}

/// Aggregate trade event from symbol aggTrade stream
#[derive(Debug, Deserialize, Clone)]
pub struct AggTradeEvent {
    #[serde(rename = "e")]
    #[allow(dead_code)]
    pub event_type: String,      // "aggTrade"

    #[serde(rename = "E")]
    #[allow(dead_code)]
    pub event_time: u64,          // Event time

    #[serde(rename = "s")]
    #[allow(dead_code)]
    pub symbol: String,           // Symbol

    #[serde(rename = "a")]
    #[allow(dead_code)]
    pub agg_trade_id: u64,        // Aggregate trade ID

    #[serde(rename = "p")]
    #[allow(dead_code)]
    pub price: String,            // Price

    #[serde(rename = "q")]
    #[allow(dead_code)]
    pub quantity: String,         // Quantity

    #[serde(rename = "f")]
    #[allow(dead_code)]
    pub first_trade_id: u64,      // First trade ID

    #[serde(rename = "l")]
    #[allow(dead_code)]
    pub last_trade_id: u64,       // Last trade ID

    #[serde(rename = "T")]
    #[allow(dead_code)]
    pub trade_time: u64,          // Trade time

    #[serde(rename = "m")]
    #[allow(dead_code)]
    pub is_buyer_maker: bool,     // Is buyer the market maker?
}

/// 24hr ticker statistics
#[derive(Debug, Deserialize, Clone)]
pub struct TickerEvent {
    #[serde(rename = "e")]
    #[allow(dead_code)]
    pub event_type: String,       // "24hrTicker"

    #[serde(rename = "E")]
    #[allow(dead_code)]
    pub event_time: u64,           // Event time

    #[serde(rename = "s")]
    #[allow(dead_code)]
    pub symbol: String,            // Symbol

    #[serde(rename = "p")]
    #[allow(dead_code)]
    pub price_change: String,      // Price change

    #[serde(rename = "P")]
    #[allow(dead_code)]
    pub price_change_percent: String, // Price change percent

    #[serde(rename = "w")]
    #[allow(dead_code)]
    pub weighted_avg_price: String,   // Weighted average price

    #[serde(rename = "c")]
    #[allow(dead_code)]
    pub last_price: String,        // Last price

    #[serde(rename = "Q")]
    #[allow(dead_code)]
    pub last_quantity: String,     // Last quantity

    #[serde(rename = "o")]
    #[allow(dead_code)]
    pub open_price: String,        // Open price

    #[serde(rename = "h")]
    #[allow(dead_code)]
    pub high_price: String,        // High price

    #[serde(rename = "l")]
    #[allow(dead_code)]
    pub low_price: String,         // Low price

    #[serde(rename = "v")]
    #[allow(dead_code)]
    pub volume: String,            // Total traded base asset volume

    #[serde(rename = "q")]
    #[allow(dead_code)]
    pub quote_volume: String,      // Total traded quote asset volume

    #[serde(rename = "O")]
    #[allow(dead_code)]
    pub open_time: u64,            // Statistics open time

    #[serde(rename = "C")]
    #[allow(dead_code)]
    pub close_time: u64,           // Statistics close time

    #[serde(rename = "F")]
    #[allow(dead_code)]
    pub first_trade_id: i64,       // First trade ID

    #[serde(rename = "L")]
    #[allow(dead_code)]
    pub last_trade_id: i64,        // Last trade ID

    #[serde(rename = "n")]
    #[allow(dead_code)]
    pub count: u64,                // Total number of trades
}

/// Subscription/Unsubscription request
#[derive(Debug, Serialize)]
pub struct SubscriptionRequest {
    pub method: String,
    pub params: Vec<String>,
    pub id: u64,
}

/// Subscription response
#[derive(Debug, Deserialize)]
pub struct SubscriptionResponse {
    #[allow(dead_code)]
    pub result: Option<Value>,
    #[allow(dead_code)]
    pub id: u64,
}

/// Enum for different message types
#[derive(Debug)]
#[allow(dead_code)]
pub enum BinanceMessage {
    Trade(TradeEvent),
    AggTrade(AggTradeEvent),
    Ticker(TickerEvent),
    Subscription(SubscriptionResponse),
    Unknown(Value),
}

/// 24hr Ticker statistics from REST API
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Ticker24hr {
    #[allow(dead_code)]
    pub symbol: String,
    #[allow(dead_code)]
    pub price_change: String,
    #[allow(dead_code)]
    pub price_change_percent: String,
    #[serde(rename = "weightedAvgPrice")]
    #[allow(dead_code)]
    pub weighted_avg_price: Option<String>,
    #[allow(dead_code)]
    pub prev_close_price: String,
    #[allow(dead_code)]
    pub last_price: String,
    #[serde(rename = "lastQty")]
    #[allow(dead_code)]
    pub last_qty: Option<String>,
    #[allow(dead_code)]
    pub bid_price: String,
    #[allow(dead_code)]
    pub bid_qty: String,
    #[allow(dead_code)]
    pub ask_price: String,
    #[allow(dead_code)]
    pub ask_qty: String,
    #[allow(dead_code)]
    pub open_price: String,
    #[allow(dead_code)]
    pub high_price: String,
    #[allow(dead_code)]
    pub low_price: String,
    #[allow(dead_code)]
    pub volume: String,
    #[allow(dead_code)]
    pub quote_volume: String,
    #[allow(dead_code)]
    pub open_time: i64,
    #[allow(dead_code)]
    pub close_time: i64,
    #[allow(dead_code)]
    pub first_id: i64,
    #[allow(dead_code)]
    pub last_id: i64,
    #[allow(dead_code)]
    pub count: i64,
}

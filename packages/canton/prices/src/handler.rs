use chrono::{DateTime, Local, Utc};
use serde_json::Value;
use tracing::{debug, warn};

use crate::models::{AggTradeEvent, BinanceMessage, StreamMessage, TickerEvent, TradeEvent};

/// Parse and handle Binance WebSocket messages
pub fn parse_message(text: &str) -> Result<BinanceMessage, Box<dyn std::error::Error>> {
    let value: Value = serde_json::from_str(text)?;

    // Check if it's a subscription response
    if value.get("id").is_some() && value.get("result").is_some() {
        let response = serde_json::from_value(value.clone())?;
        return Ok(BinanceMessage::Subscription(response));
    }

    // Check if it's a stream message (from combined streams)
    if let Ok(stream_msg) = serde_json::from_str::<StreamMessage>(text) {
        return parse_stream_data(&stream_msg.stream, stream_msg.data);
    }

    // Try to parse as direct message (single stream)
    if let Some(event_type) = value.get("e").and_then(|e| e.as_str()) {
        match event_type {
            "trade" => {
                let trade = serde_json::from_value(value)?;
                return Ok(BinanceMessage::Trade(trade));
            }
            "aggTrade" => {
                let agg_trade = serde_json::from_value(value)?;
                return Ok(BinanceMessage::AggTrade(agg_trade));
            }
            "24hrTicker" => {
                let ticker = serde_json::from_value(value)?;
                return Ok(BinanceMessage::Ticker(ticker));
            }
            _ => {
                debug!("Unknown event type: {}", event_type);
            }
        }
    }

    Ok(BinanceMessage::Unknown(value))
}

/// Parse data from a stream message
fn parse_stream_data(stream: &str, data: Value) -> Result<BinanceMessage, Box<dyn std::error::Error>> {
    if stream.ends_with("@trade") {
        let trade = serde_json::from_value(data)?;
        Ok(BinanceMessage::Trade(trade))
    } else if stream.ends_with("@aggTrade") {
        let agg_trade = serde_json::from_value(data)?;
        Ok(BinanceMessage::AggTrade(agg_trade))
    } else if stream.ends_with("@ticker") {
        let ticker = serde_json::from_value(data)?;
        Ok(BinanceMessage::Ticker(ticker))
    } else {
        Ok(BinanceMessage::Unknown(data))
    }
}

/// Format and print the message to console
pub fn print_message(message: BinanceMessage) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

    match message {
        BinanceMessage::Trade(trade) => {
            let time = DateTime::<Utc>::from_timestamp_millis(trade.event_time as i64)
                .map(|dt| dt.with_timezone(&Local))
                .map(|dt| dt.format("%H:%M:%S%.3f").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            println!(
                "[{}] {} | {}",
                timestamp,
                format_trade(&trade),
                time
            );
        }
        BinanceMessage::AggTrade(agg_trade) => {
            let time = DateTime::<Utc>::from_timestamp_millis(agg_trade.event_time as i64)
                .map(|dt| dt.with_timezone(&Local))
                .map(|dt| dt.format("%H:%M:%S%.3f").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            println!(
                "[{}] {} | {}",
                timestamp,
                format_agg_trade(&agg_trade),
                time
            );
        }
        BinanceMessage::Ticker(ticker) => {
            println!(
                "[{}] {}",
                timestamp,
                format_ticker(&ticker)
            );
        }
        BinanceMessage::Subscription(response) => {
            debug!("Subscription response: {:?}", response);
        }
        BinanceMessage::Unknown(value) => {
            warn!("Unknown message: {}", value);
        }
    }
}

/// Format a trade event for display
fn format_trade(trade: &TradeEvent) -> String {
    let side = if trade.is_buyer_maker {
        "SELL"
    } else {
        "BUY"
    };

    let symbol = format_symbol(&trade.symbol);

    format!(
        "{} TRADE: ${} | Qty: {} | Side: {} | ID: {}",
        symbol,
        trade.price,
        trade.quantity,
        side,
        trade.trade_id
    )
}

/// Format an aggregate trade event for display
fn format_agg_trade(agg_trade: &AggTradeEvent) -> String {
    let side = if agg_trade.is_buyer_maker {
        "SELL"
    } else {
        "BUY"
    };

    let symbol = format_symbol(&agg_trade.symbol);

    format!(
        "{} AGG-TRADE: ${} | Qty: {} | Side: {}",
        symbol,
        agg_trade.price,
        agg_trade.quantity,
        side
    )
}

/// Format a ticker event for display
fn format_ticker(ticker: &TickerEvent) -> String {
    let symbol = format_symbol(&ticker.symbol);

    let change_symbol = if ticker.price_change_percent.starts_with('-') {
        ""
    } else {
        "+"
    };

    format!(
        "{} TICKER: ${} | 24h: {}{}% | High: ${} | Low: ${} | Vol: {}",
        symbol,
        ticker.last_price,
        change_symbol,
        ticker.price_change_percent,
        ticker.high_price,
        ticker.low_price,
        ticker.volume
    )
}

/// Format symbol for display (e.g., "btcusdt" -> "BTC/USDT")
fn format_symbol(symbol: &str) -> String {
    let upper = symbol.to_uppercase();

    // Common USDT pairs
    if upper.ends_with("USDT") {
        let base = &upper[..upper.len() - 4];
        return format!("{}/USDT", base);
    }

    // Common USDC pairs
    if upper.ends_with("USDC") {
        let base = &upper[..upper.len() - 4];
        return format!("{}/USDC", base);
    }

    // Common BTC pairs
    if upper.ends_with("BTC") {
        let base = &upper[..upper.len() - 3];
        return format!("{}/BTC", base);
    }

    upper
}

/// Statistics tracker for monitoring
pub struct Statistics {
    pub total_messages: u64,
    pub trades_received: u64,
    pub tickers_received: u64,
    pub errors: u64,
    pub start_time: std::time::Instant,
}

impl Statistics {
    pub fn new() -> Self {
        Self {
            total_messages: 0,
            trades_received: 0,
            tickers_received: 0,
            errors: 0,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, message: &BinanceMessage) {
        self.total_messages += 1;
        match message {
            BinanceMessage::Trade(_) | BinanceMessage::AggTrade(_) => self.trades_received += 1,
            BinanceMessage::Ticker(_) => self.tickers_received += 1,
            _ => {}
        }
    }

    pub fn increment_errors(&mut self) {
        self.errors += 1;
    }

    pub fn print_summary(&self) {
        let elapsed = self.start_time.elapsed();
        println!(
            "\n--- Statistics ---\n\
             Runtime: {:?}\n\
             Total messages: {}\n\
             Trades: {}\n\
             Tickers: {}\n\
             Errors: {}\n\
             Avg rate: {:.2} msg/sec",
            elapsed,
            self.total_messages,
            self.trades_received,
            self.tickers_received,
            self.errors,
            self.total_messages as f64 / elapsed.as_secs_f64()
        );
    }
}
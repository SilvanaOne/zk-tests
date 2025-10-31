use chrono::{DateTime, Local, Utc};
use serde_json::Value;
use tracing::{debug, warn};

use super::models::{BybitMessage, BybitWsMessage, SubscriptionResponse, TradeData, TickerData};

/// Parse and handle Bybit WebSocket messages
pub fn parse_message(text: &str) -> Result<BybitMessage, Box<dyn std::error::Error>> {
    let ws_msg: BybitWsMessage = serde_json::from_str(text)?;

    // Handle pong responses
    if let Some(op) = &ws_msg.op {
        if op == "pong" || op == "ping" {
            return Ok(BybitMessage::Pong);
        }

        // Handle subscription responses
        if op == "subscribe" {
            let response = SubscriptionResponse {
                success: ws_msg.success.unwrap_or(false),
                op: op.clone(),
                ret_msg: ws_msg.ret_msg,
                conn_id: ws_msg.conn_id,
            };
            return Ok(BybitMessage::Subscription(response));
        }
    }

    // Handle data messages
    if let Some(topic) = &ws_msg.topic {
        if topic.starts_with("publicTrade.") {
            // Parse trade data
            if let Some(data) = ws_msg.data {
                let trades: Vec<TradeData> = serde_json::from_value(data)?;
                return Ok(BybitMessage::Trade(trades));
            }
        } else if topic.starts_with("tickers.") {
            // Parse ticker data
            if let Some(data) = ws_msg.data {
                let ticker: TickerData = serde_json::from_value(data)?;
                return Ok(BybitMessage::Ticker(ticker));
            }
        }
    }

    // Unknown message type
    let value: Value = serde_json::from_str(text)?;
    Ok(BybitMessage::Unknown(value))
}

/// Format and print the message to console
pub fn print_message(message: BybitMessage) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

    match message {
        BybitMessage::Trade(trades) => {
            for trade in trades {
                let time = DateTime::<Utc>::from_timestamp_millis(trade.timestamp as i64)
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
        }
        BybitMessage::Ticker(ticker) => {
            println!(
                "[{}] {}",
                timestamp,
                format_ticker(&ticker)
            );
        }
        BybitMessage::Subscription(response) => {
            debug!("Subscription response: {:?}", response);
        }
        BybitMessage::Pong => {
            debug!("Received pong");
        }
        BybitMessage::Unknown(value) => {
            warn!("Unknown message: {}", value);
        }
    }
}

/// Format a trade event for display
fn format_trade(trade: &TradeData) -> String {
    let side = match trade.side.as_str() {
        "Buy" => "BUY",
        "Sell" => "SELL",
        _ => "UNKNOWN",
    };

    let symbol = format_symbol(&trade.symbol);

    format!(
        "{} TRADE: ${} | Qty: {} | Side: {} | ID: {}",
        symbol,
        trade.price,
        trade.volume,
        side,
        trade.trade_id
    )
}

/// Format a ticker event for display
fn format_ticker(ticker: &TickerData) -> String {
    let symbol = format_symbol(&ticker.symbol);

    let change_percent = &ticker.price_24h_pcnt;
    let change_symbol = if change_percent.starts_with('-') {
        ""
    } else {
        "+"
    };

    format!(
        "{} TICKER: ${} | 24h: {}{}% | High: ${} | Low: ${} | Vol: {}",
        symbol,
        ticker.last_price,
        change_symbol,
        change_percent,
        ticker.high_price_24h,
        ticker.low_price_24h,
        ticker.volume_24h
    )
}

/// Format symbol for display (e.g., "BTCUSDT" -> "BTC/USDT")
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

    pub fn update(&mut self, message: &BybitMessage) {
        self.total_messages += 1;
        match message {
            BybitMessage::Trade(_) => self.trades_received += 1,
            BybitMessage::Ticker(_) => self.tickers_received += 1,
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

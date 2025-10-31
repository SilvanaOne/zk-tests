use anyhow::Result;
use async_trait::async_trait;
use futures_util::SinkExt;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::provider::ProviderWebSocket;
use super::models::{SubscriptionRequest, PingMessage};

pub struct BybitWebSocket {
    url: String,
    symbols: Vec<String>,
    reconnect_delay: Duration,
    max_reconnect_delay: Duration,
}

impl BybitWebSocket {
    /// Create a new Bybit WebSocket client
    pub fn new(symbols: Vec<String>) -> Self {
        let url = "wss://stream.bybit.com/v5/public/spot".to_string();

        Self {
            url,
            symbols,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
        }
    }

    /// Establish WebSocket connection and subscribe to topics
    async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        info!("Connecting to {}", &self.url);

        let (mut ws_stream, response) = connect_async(&self.url).await?;
        debug!("WebSocket handshake completed with status: {}", response.status());

        // Subscribe to topics after connection
        let mut topics = Vec::new();
        for symbol in &self.symbols {
            topics.push(format!("publicTrade.{}", symbol));
            topics.push(format!("tickers.{}", symbol));
        }

        let subscription = SubscriptionRequest {
            op: "subscribe".to_string(),
            args: topics,
            req_id: None,
        };

        let subscription_msg = serde_json::to_string(&subscription)?;
        ws_stream.send(Message::Text(subscription_msg)).await?;
        info!("Sent subscription request for {} symbols", self.symbols.len());

        Ok(ws_stream)
    }
}

#[async_trait]
impl ProviderWebSocket for BybitWebSocket {
    fn new(symbols: Vec<String>) -> Self {
        BybitWebSocket::new(symbols)
    }

    /// Connect to the WebSocket with automatic reconnection
    async fn connect_with_retry(&mut self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        let mut delay = self.reconnect_delay;

        loop {
            match self.connect().await {
                Ok(ws_stream) => {
                    info!("Successfully connected to Bybit WebSocket");
                    self.reconnect_delay = Duration::from_secs(1); // Reset delay on success
                    return Ok(ws_stream);
                }
                Err(e) => {
                    error!("Failed to connect: {}. Retrying in {:?}...", e, delay);
                    sleep(delay).await;

                    // Exponential backoff
                    delay = std::cmp::min(delay * 2, self.max_reconnect_delay);
                }
            }
        }
    }

    fn provider_name(&self) -> &str {
        "bybit"
    }
}

/// WebSocket message handler
pub struct MessageHandler {
    last_message_time: std::time::Instant,
    last_ping_time: std::time::Instant,
}

impl MessageHandler {
    pub fn new() -> Self {
        Self {
            last_message_time: std::time::Instant::now(),
            last_ping_time: std::time::Instant::now(),
        }
    }

    /// Update last message time for health monitoring
    pub fn update_last_message_time(&mut self) {
        self.last_message_time = std::time::Instant::now();
    }

    /// Check if connection is healthy (received message within timeout)
    pub fn is_healthy(&self, timeout: Duration) -> bool {
        self.last_message_time.elapsed() < timeout
    }

    /// Check if we need to send a ping (every 20 seconds)
    pub fn should_send_ping(&self) -> bool {
        self.last_ping_time.elapsed() >= Duration::from_secs(20)
    }

    /// Send ping to keep connection alive
    pub async fn send_ping<S>(&mut self, ws_stream: &mut S) -> Result<()>
    where
        S: SinkExt<Message> + Unpin,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let ping = PingMessage {
            op: "ping".to_string(),
        };
        let ping_msg = serde_json::to_string(&ping)?;
        ws_stream.send(Message::Text(ping_msg)).await
            .map_err(|e| anyhow::anyhow!("Send error: {}", Into::<Box<dyn std::error::Error + Send + Sync>>::into(e)))?;
        self.last_ping_time = std::time::Instant::now();
        debug!("Sent ping");
        Ok(())
    }

    /// Process incoming WebSocket message
    pub async fn process_message(&mut self, msg: Message) -> Result<Option<String>> {
        self.update_last_message_time();

        match msg {
            Message::Text(text) => {
                Ok(Some(text))
            }
            Message::Binary(bin) => {
                let text = String::from_utf8(bin)?;
                Ok(Some(text))
            }
            Message::Ping(_data) => {
                debug!("Received ping");
                Ok(None)
            }
            Message::Pong(_) => {
                debug!("Received pong");
                Ok(None)
            }
            Message::Close(frame) => {
                warn!("WebSocket close frame received: {:?}", frame);
                anyhow::bail!("Connection closed by server")
            }
            Message::Frame(_) => {
                Ok(None)
            }
        }
    }
}

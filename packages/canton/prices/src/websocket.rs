use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

pub struct BinanceWebSocket {
    url: String,
    #[allow(dead_code)]
    symbols: Vec<String>,
    reconnect_delay: Duration,
    max_reconnect_delay: Duration,
}

impl BinanceWebSocket {
    /// Create a new Binance WebSocket client
    pub fn new(symbols: Vec<String>) -> Self {
        // Using combined streams endpoint for multiple symbols
        let streams = symbols
            .iter()
            .flat_map(|symbol| {
                let symbol_lower = symbol.to_lowercase();
                vec![
                    format!("{}@trade", symbol_lower),
                    format!("{}@ticker", symbol_lower),
                ]
            })
            .collect::<Vec<_>>()
            .join("/");

        let url = format!("wss://stream.binance.com:9443/stream?streams={}", streams);

        Self {
            url,
            symbols,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
        }
    }

    /// Connect to the WebSocket with automatic reconnection
    pub async fn connect_with_retry(&mut self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error>> {
        let mut delay = self.reconnect_delay;

        loop {
            match self.connect().await {
                Ok(ws_stream) => {
                    info!("Successfully connected to Binance WebSocket");
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

    /// Establish WebSocket connection
    async fn connect(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error>> {
        info!("Connecting to {}", &self.url);

        let (ws_stream, response) = connect_async(&self.url).await?;
        debug!("WebSocket handshake completed with status: {}", response.status());

        Ok(ws_stream)
    }

}

/// WebSocket message handler
pub struct MessageHandler {
    last_message_time: std::time::Instant,
}

impl MessageHandler {
    pub fn new() -> Self {
        Self {
            last_message_time: std::time::Instant::now(),
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

    /// Process incoming WebSocket message
    pub async fn process_message(&mut self, msg: Message) -> Result<Option<String>, Box<dyn std::error::Error>> {
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
                Err("Connection closed by server".into())
            }
            Message::Frame(_) => {
                Ok(None)
            }
        }
    }
}
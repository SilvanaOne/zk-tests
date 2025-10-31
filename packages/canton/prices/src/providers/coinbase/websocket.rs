use anyhow::Result;
use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::provider::ProviderWebSocket;

pub struct CoinbaseWebSocket {
    #[allow(dead_code)]
    symbols: Vec<String>,
}

#[async_trait]
impl ProviderWebSocket for CoinbaseWebSocket {
    fn new(symbols: Vec<String>) -> Self {
        Self { symbols }
    }

    async fn connect_with_retry(
        &mut self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
        unimplemented!("Coinbase WebSocket support coming soon")
    }

    fn provider_name(&self) -> &str {
        "coinbase"
    }
}

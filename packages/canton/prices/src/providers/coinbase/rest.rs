use anyhow::Result;
use async_trait::async_trait;

use crate::provider::{ProviderRest, TickerData};

pub struct CoinbaseRestClient;

#[async_trait]
impl ProviderRest for CoinbaseRestClient {
    fn new() -> Self {
        Self
    }

    async fn get_24hr_ticker(
        &self,
        _symbols: &[String],
    ) -> Result<Vec<TickerData>> {
        unimplemented!("Coinbase REST API support coming soon")
    }

    fn provider_name(&self) -> &str {
        "coinbase"
    }
}

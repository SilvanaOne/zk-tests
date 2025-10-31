use anyhow::Result;
use async_trait::async_trait;
use reqwest;
use tracing::{debug, info};

use crate::provider::{ProviderRest, TickerData};
use super::models::{TickersResponse, TickerRestData};

/// Bybit REST API client
pub struct BybitRestClient {
    base_url: String,
    client: reqwest::Client,
}

impl BybitRestClient {
    /// Create a new Bybit REST API client
    pub fn new() -> Self {
        Self {
            base_url: "https://api.bybit.com".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Convert Bybit TickerRestData to common TickerData
    fn to_ticker_data(ticker: &TickerRestData) -> TickerData {
        // Calculate price change from price_24h_pcnt and last_price
        let price_change = if let (Ok(last_price), Ok(percent)) =
            (ticker.last_price.parse::<f64>(), ticker.price_24h_pcnt.parse::<f64>()) {
            let change = last_price * percent / 100.0;
            format!("{:.8}", change)
        } else {
            "0".to_string()
        };

        // Calculate open price from last price and percent change
        let open_price = if let (Ok(last_price), Ok(percent)) =
            (ticker.last_price.parse::<f64>(), ticker.price_24h_pcnt.parse::<f64>()) {
            let open = last_price / (1.0 + percent / 100.0);
            format!("{:.8}", open)
        } else {
            ticker.last_price.clone()
        };

        TickerData {
            symbol: ticker.symbol.clone(),
            last_price: ticker.last_price.clone(),
            price_change,
            price_change_percent: ticker.price_24h_pcnt.clone(),
            high_price: ticker.high_price_24h.clone(),
            low_price: ticker.low_price_24h.clone(),
            volume: ticker.volume_24h.clone(),
            quote_volume: ticker.turnover_24h.clone(),
            bid_price: ticker.bid1_price.clone(),
            bid_qty: ticker.bid1_size.clone(),
            ask_price: ticker.ask1_price.clone(),
            ask_qty: ticker.ask1_size.clone(),
            open_price,
            prev_close_price: ticker.prev_price_24h.clone().unwrap_or_else(|| "0".to_string()),
            open_time: 0,  // Bybit doesn't provide this
            close_time: 0,  // Bybit doesn't provide this
            count: 0,  // Bybit doesn't provide this
        }
    }

    /// Fetch ticker for a single symbol
    async fn get_symbol_ticker(&self, symbol: &str) -> Result<TickerRestData> {
        let url = format!("{}/v5/market/tickers", self.base_url);

        debug!("Fetching ticker for symbol: {}", symbol);

        let response = self.client
            .get(&url)
            .query(&[("category", "spot"), ("symbol", symbol)])
            .send()
            .await?;

        debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("API error for {}: {}", symbol, error_text);
        }

        let text = response.text().await?;
        debug!("Response body: {}", &text[..text.len().min(200)]);

        let ticker_response: TickersResponse = serde_json::from_str(&text)?;

        if ticker_response.ret_code != 0 {
            anyhow::bail!("API returned error code {} for {}: {}",
                ticker_response.ret_code, symbol, ticker_response.ret_msg);
        }

        if ticker_response.result.list.is_empty() {
            anyhow::bail!("No ticker data found for symbol: {}", symbol);
        }

        Ok(ticker_response.result.list[0].clone())
    }
}

#[async_trait]
impl ProviderRest for BybitRestClient {
    fn new() -> Self {
        BybitRestClient::new()
    }

    /// Fetch 24hr ticker data for one or more symbols
    /// Note: Bybit doesn't support batch requests, so we fetch one at a time
    async fn get_24hr_ticker(
        &self,
        symbols: &[String],
    ) -> Result<Vec<TickerData>> {
        if symbols.is_empty() {
            anyhow::bail!("No symbols provided");
        }

        info!("Fetching 24hr ticker data for {} symbols", symbols.len());

        let mut tickers = Vec::new();

        // Fetch each symbol individually
        for symbol in symbols {
            match self.get_symbol_ticker(symbol).await {
                Ok(ticker) => {
                    tickers.push(Self::to_ticker_data(&ticker));
                }
                Err(e) => {
                    // Log error but continue with other symbols
                    tracing::error!("Failed to fetch ticker for {}: {}", symbol, e);
                }
            }
        }

        if tickers.is_empty() {
            anyhow::bail!("Failed to fetch any ticker data");
        }

        info!("Successfully fetched {} ticker(s)", tickers.len());

        Ok(tickers)
    }

    fn provider_name(&self) -> &str {
        "bybit"
    }
}

use anyhow::Result;
use async_trait::async_trait;
use reqwest;
use tracing::{debug, info};

use crate::provider::{ProviderRest, TickerData};
use super::models::Ticker24hr;

/// Binance REST API client
pub struct BinanceRestClient {
    base_url: String,
    client: reqwest::Client,
}

impl BinanceRestClient {
    /// Create a new Binance REST API client
    pub fn new() -> Self {
        Self {
            base_url: "https://api.binance.com".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Convert Binance Ticker24hr to common TickerData
    fn to_ticker_data(ticker: &Ticker24hr) -> TickerData {
        TickerData {
            symbol: ticker.symbol.clone(),
            last_price: ticker.last_price.clone(),
            price_change: ticker.price_change.clone(),
            price_change_percent: ticker.price_change_percent.clone(),
            high_price: ticker.high_price.clone(),
            low_price: ticker.low_price.clone(),
            volume: ticker.volume.clone(),
            quote_volume: ticker.quote_volume.clone(),
            bid_price: ticker.bid_price.clone(),
            bid_qty: ticker.bid_qty.clone(),
            ask_price: ticker.ask_price.clone(),
            ask_qty: ticker.ask_qty.clone(),
            open_price: ticker.open_price.clone(),
            prev_close_price: ticker.prev_close_price.clone(),
            open_time: ticker.open_time,
            close_time: ticker.close_time,
            count: ticker.count,
        }
    }
}

#[async_trait]
impl ProviderRest for BinanceRestClient {
    fn new() -> Self {
        BinanceRestClient::new()
    }

    /// Fetch 24hr ticker data for one or more symbols
    async fn get_24hr_ticker(
        &self,
        symbols: &[String],
    ) -> Result<Vec<TickerData>> {
        let url = format!("{}/api/v3/ticker/24hr", self.base_url);

        info!("Fetching 24hr ticker data for {} symbols", symbols.len());
        debug!("Request URL: {}", url);

        let response = if symbols.len() == 1 {
            // Single symbol query
            self.client
                .get(&url)
                .query(&[("symbol", &symbols[0])])
                .send()
                .await?
        } else if symbols.len() > 1 {
            // Multiple symbols query - need to format as JSON array
            let symbols_json = serde_json::to_string(symbols)?;
            self.client
                .get(&url)
                .query(&[("symbols", symbols_json)])
                .send()
                .await?
        } else {
            // No symbols - return all (but we'll avoid this for now)
            anyhow::bail!("No symbols provided");
        };

        debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("API error: {}", error_text);
        }

        // Parse response - could be single object or array
        let text = response.text().await?;
        debug!("Response body: {}", &text[..text.len().min(200)]);

        let tickers: Vec<Ticker24hr> = if symbols.len() == 1 {
            // Single symbol returns an object, wrap in Vec
            let ticker: Ticker24hr = serde_json::from_str(&text)?;
            vec![ticker]
        } else {
            // Multiple symbols returns an array
            serde_json::from_str(&text)?
        };

        info!("Successfully fetched {} ticker(s)", tickers.len());

        // Convert to common TickerData format
        Ok(tickers.iter().map(|t| Self::to_ticker_data(t)).collect())
    }

    fn provider_name(&self) -> &str {
        "binance"
    }
}

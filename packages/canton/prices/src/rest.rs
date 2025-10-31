use reqwest;
use tracing::{debug, info};

use crate::models::Ticker24hr;

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

    /// Fetch 24hr ticker data for one or more symbols
    pub async fn get_24hr_ticker(
        &self,
        symbols: &[String],
    ) -> Result<Vec<Ticker24hr>, Box<dyn std::error::Error>> {
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
            return Err("No symbols provided".into());
        };

        debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("API error: {}", error_text).into());
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
        Ok(tickers)
    }
}

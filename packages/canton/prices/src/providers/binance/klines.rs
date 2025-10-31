use anyhow::Result;
use reqwest;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Binance Kline (candlestick) REST API client
pub struct BinanceKlineClient {
    base_url: String,
    client: reqwest::Client,
}

/// Kline data from Binance
/// Format: [
///   open_time, open, high, low, close, volume,
///   close_time, quote_asset_volume, number_of_trades,
///   taker_buy_base_asset_volume, taker_buy_quote_asset_volume, ignore
/// ]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KlineData {
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub close_time: i64,
    pub quote_asset_volume: String,
    pub number_of_trades: i64,
    pub taker_buy_base_asset_volume: String,
    pub taker_buy_quote_asset_volume: String,
    #[serde(skip)]
    #[allow(dead_code)]
    pub ignore: String,
}

impl BinanceKlineClient {
    /// Create a new Binance Kline API client
    pub fn new() -> Self {
        Self {
            base_url: "https://api.binance.com".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch kline/candlestick data for a symbol
    ///
    /// # Arguments
    /// * `symbol` - Trading pair (e.g., "BTCUSDT")
    /// * `interval` - Kline interval (e.g., "5m", "15m", "1h", "1d")
    /// * `limit` - Number of klines to fetch (default: 1 for latest closed candle)
    ///
    /// # Intervals
    /// - 1m, 3m, 5m, 15m, 30m
    /// - 1h, 2h, 4h, 6h, 8h, 12h
    /// - 1d, 3d, 1w, 1M
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: Option<u32>,
    ) -> Result<Vec<KlineData>> {
        let url = format!("{}/api/v3/klines", self.base_url);
        let limit = limit.unwrap_or(1);

        info!("Fetching {} klines for {} (interval: {})", limit, symbol, interval);
        debug!("Request URL: {}", url);

        let response = self.client
            .get(&url)
            .query(&[
                ("symbol", symbol),
                ("interval", interval),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?;

        debug!("Response status: {}", response.status());

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Binance API error: {}", error_text);
        }

        let text = response.text().await?;
        debug!("Response body (first 200 chars): {}", &text[..text.len().min(200)]);

        // Parse response as array of arrays
        let raw_klines: Vec<serde_json::Value> = serde_json::from_str(&text)?;

        // Convert to KlineData structs
        let mut klines = Vec::new();
        for raw in raw_klines {
            if let Some(arr) = raw.as_array() {
                if arr.len() >= 11 {
                    let kline = KlineData {
                        open_time: arr[0].as_i64().unwrap_or(0),
                        open: arr[1].as_str().unwrap_or("0").to_string(),
                        high: arr[2].as_str().unwrap_or("0").to_string(),
                        low: arr[3].as_str().unwrap_or("0").to_string(),
                        close: arr[4].as_str().unwrap_or("0").to_string(),
                        volume: arr[5].as_str().unwrap_or("0").to_string(),
                        close_time: arr[6].as_i64().unwrap_or(0),
                        quote_asset_volume: arr[7].as_str().unwrap_or("0").to_string(),
                        number_of_trades: arr[8].as_i64().unwrap_or(0),
                        taker_buy_base_asset_volume: arr[9].as_str().unwrap_or("0").to_string(),
                        taker_buy_quote_asset_volume: arr[10].as_str().unwrap_or("0").to_string(),
                        ignore: String::new(),
                    };
                    klines.push(kline);
                }
            }
        }

        info!("Successfully fetched {} klines", klines.len());
        Ok(klines)
    }

    /// Get the latest closed kline for a symbol
    pub async fn get_latest_kline(
        &self,
        symbol: &str,
        interval: &str,
    ) -> Result<KlineData> {
        let klines = self.get_klines(symbol, interval, Some(2)).await?;

        // Return the second-to-last kline (the last one might not be closed yet)
        // If we only got 1, return that one
        if klines.len() >= 2 {
            Ok(klines[klines.len() - 2].clone())
        } else if !klines.is_empty() {
            Ok(klines[0].clone())
        } else {
            anyhow::bail!("No klines returned from Binance API")
        }
    }
}

impl KlineData {
    /// Parse open price as f64
    pub fn open_f64(&self) -> Result<f64> {
        self.open.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse open price: {}", e))
    }

    /// Parse high price as f64
    pub fn high_f64(&self) -> Result<f64> {
        self.high.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse high price: {}", e))
    }

    /// Parse low price as f64
    pub fn low_f64(&self) -> Result<f64> {
        self.low.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse low price: {}", e))
    }

    /// Parse close price as f64
    pub fn close_f64(&self) -> Result<f64> {
        self.close.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse close price: {}", e))
    }

    /// Parse volume as f64
    #[allow(dead_code)]
    pub fn volume_f64(&self) -> Result<f64> {
        self.volume.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse volume: {}", e))
    }

    /// Convert to OHLC tuple (open, high, low, close)
    pub fn to_ohlc(&self) -> Result<(f64, f64, f64, f64)> {
        Ok((
            self.open_f64()?,
            self.high_f64()?,
            self.low_f64()?,
            self.close_f64()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kline_parsing() {
        let kline = KlineData {
            open_time: 1699920000000,
            open: "35000.50".to_string(),
            high: "35500.75".to_string(),
            low: "34800.25".to_string(),
            close: "35200.00".to_string(),
            volume: "1234.567".to_string(),
            close_time: 1699923600000,
            quote_asset_volume: "43456789.12".to_string(),
            number_of_trades: 15000,
            taker_buy_base_asset_volume: "600.0".to_string(),
            taker_buy_quote_asset_volume: "21000000.0".to_string(),
            ignore: String::new(),
        };

        assert_eq!(kline.open_f64().unwrap(), 35000.50);
        assert_eq!(kline.high_f64().unwrap(), 35500.75);
        assert_eq!(kline.low_f64().unwrap(), 34800.25);
        assert_eq!(kline.close_f64().unwrap(), 35200.00);

        let (o, h, l, c) = kline.to_ohlc().unwrap();
        assert_eq!(o, 35000.50);
        assert_eq!(h, 35500.75);
        assert_eq!(l, 34800.25);
        assert_eq!(c, 35200.00);
    }
}

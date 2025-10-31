pub mod binance;
pub mod bybit;
pub mod coinbase;

use crate::provider::ProviderType;

/// Get token symbols for a provider
pub fn get_provider_symbols(provider: ProviderType, tokens: &[String]) -> Vec<String> {
    match provider {
        ProviderType::Binance => {
            // Binance uses USDT pairs
            tokens
                .iter()
                .map(|token| {
                    let token_upper = token.to_uppercase();
                    format!("{}USDT", token_upper)
                })
                .collect()
        }
        ProviderType::Bybit => {
            // Bybit uses USDT pairs (when implemented)
            tokens
                .iter()
                .map(|token| {
                    let token_upper = token.to_uppercase();
                    format!("{}USDT", token_upper)
                })
                .collect()
        }
        ProviderType::Coinbase => {
            // Coinbase uses different format like BTC-USD (when implemented)
            tokens
                .iter()
                .map(|token| {
                    let token_upper = token.to_uppercase();
                    format!("{}-USD", token_upper)
                })
                .collect()
        }
    }
}

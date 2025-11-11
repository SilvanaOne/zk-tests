mod models;
mod news;
mod pivot;
mod provider;
mod providers;

use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use std::collections::HashSet;
use std::time::Duration;
use tokio::signal;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};
use tracing_subscriber;

use crate::provider::{ProviderRest, ProviderType, ProviderWebSocket};
use crate::providers::binance::{BinanceRestClient, BinanceWebSocket};
use crate::providers::bybit::BybitWebSocket;

#[derive(Parser)]
#[command(name = "prices")]
#[command(about = "Binance WebSocket price streaming application", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get first prices for each token and exit (WebSocket)
    Get {
        /// Specific tokens to stream (btc, eth, mina). If not specified, streams all tokens.
        #[arg(long, value_delimiter = ',')]
        token: Option<Vec<String>>,
        /// Exchange provider (binance, bybit, coinbase). Default: binance
        #[arg(long, default_value = "binance")]
        provider: String,
    },
    /// Run continuous price streaming (WebSocket)
    Run {
        /// Specific tokens to stream (btc, eth, mina). If not specified, streams all tokens.
        #[arg(long, value_delimiter = ',')]
        token: Option<Vec<String>>,
        /// Exchange provider (binance, bybit, coinbase). Default: binance
        #[arg(long, default_value = "binance")]
        provider: String,
    },
    /// Fetch current prices via REST API with full 24hr statistics
    Rest {
        /// Specific tokens to fetch (btc, eth, mina). If not specified, fetches all tokens.
        #[arg(long, value_delimiter = ',')]
        token: Option<Vec<String>>,
        /// Exchange provider (binance, bybit, coinbase). Default: binance
        #[arg(long, default_value = "binance")]
        provider: String,
    },
    /// Fetch news articles for cryptocurrencies
    News {
        /// Specific tokens to fetch news for (btc, eth, mina). If not specified, fetches news for all tokens.
        #[arg(long, value_delimiter = ',')]
        token: Option<Vec<String>>,
    },
    /// Calculate pivot points for cryptocurrencies
    Pivot {
        /// Specific tokens to calculate pivots for (btc, eth, mina). If not specified, calculates for all tokens.
        #[arg(long, value_delimiter = ',')]
        token: Option<Vec<String>>,
        /// Timeframe interval (5m, 15m, 30m, 1h, 4h, 1d, 1w, 1M). Default: 15m
        #[arg(long, default_value = "15m")]
        interval: String,
        /// Market type (spot, futures). Default: spot
        #[arg(long, default_value = "spot")]
        market: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_env_filter("prices=info,warn,error")
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Get { token, provider } => {
            let provider_type = ProviderType::from_str(provider)?;
            run_get_mode(token.as_ref(), provider_type).await?;
        }
        Commands::Run { token, provider } => {
            let provider_type = ProviderType::from_str(provider)?;
            run_continuous_mode(token.as_ref(), provider_type).await?;
        }
        Commands::Rest { token, provider } => {
            let provider_type = ProviderType::from_str(provider)?;
            run_rest_mode(token.as_ref(), provider_type).await?;
        }
        Commands::News { token } => {
            run_news_mode(token.as_ref()).await?;
        }
        Commands::Pivot { token, interval, market } => {
            run_pivot_mode(token.as_ref(), interval, market).await?;
        }
    }

    Ok(())
}

/// Get first prices for each token and exit
async fn run_get_mode(tokens: Option<&Vec<String>>, provider_type: ProviderType) -> Result<(), Box<dyn std::error::Error>> {
    info!("Getting first prices for tokens from {}...", provider_type.as_str());

    let default_tokens = vec!["btc".to_string(), "eth".to_string(), "mina".to_string()];
    let tokens_list = tokens.cloned().unwrap_or(default_tokens);
    let symbols = providers::get_provider_symbols(provider_type, &tokens_list);

    println!("Fetching first prices for: {} (provider: {})", symbols.join(", "), provider_type.as_str());

    match provider_type {
        ProviderType::Binance => {
            run_get_mode_binance(symbols).await
        }
        ProviderType::Bybit => {
            run_get_mode_bybit(symbols).await
        }
        _ => Err(format!("Provider {} not yet implemented for WebSocket", provider_type.as_str()).into()),
    }
}

async fn run_get_mode_binance(symbols: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::providers::binance::{handler, MessageHandler};

    let mut client = BinanceWebSocket::new(symbols.clone());
    let mut ws_stream = client.connect_with_retry().await?;
    let mut msg_handler = MessageHandler::new();
    let mut seen_symbols: HashSet<String> = HashSet::new();
    let total_needed = symbols.len();

    while seen_symbols.len() < total_needed {
        if let Some(msg_result) = ws_stream.next().await {
            match msg_result {
                Ok(msg) => {
                    if let Ok(Some(text)) = msg_handler.process_message(msg).await {
                        if let Ok(binance_msg) = handler::parse_message(&text) {
                            let symbol = get_symbol_from_binance_message(&binance_msg);
                            if let Some(sym) = symbol {
                                if !seen_symbols.contains(&sym) {
                                    handler::print_message(binance_msg);
                                    seen_symbols.insert(sym);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    println!("\n✓ Received first prices for all requested tokens");
    Ok(())
}

async fn run_get_mode_bybit(symbols: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::providers::bybit::{handler, websocket::MessageHandler};

    let mut client = BybitWebSocket::new(symbols.clone());
    let mut ws_stream = client.connect_with_retry().await?;
    let mut msg_handler = MessageHandler::new();
    let mut seen_symbols: HashSet<String> = HashSet::new();
    let total_needed = symbols.len();

    while seen_symbols.len() < total_needed {
        tokio::select! {
            Some(msg_result) = ws_stream.next() => {
                match msg_result {
                    Ok(msg) => {
                        // Check if we need to send ping
                        if msg_handler.should_send_ping() {
                            if let Err(e) = msg_handler.send_ping(&mut ws_stream).await {
                                error!("Failed to send ping: {}", e);
                            }
                        }

                        if let Ok(Some(text)) = msg_handler.process_message(msg).await {
                            if let Ok(bybit_msg) = handler::parse_message(&text) {
                                let symbol = get_symbol_from_bybit_message(&bybit_msg);
                                if let Some(sym) = symbol {
                                    if !seen_symbols.contains(&sym) {
                                        handler::print_message(bybit_msg);
                                        seen_symbols.insert(sym);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        return Err(e.into());
                    }
                }
            }
            // Periodic ping check
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                if msg_handler.should_send_ping() {
                    if let Err(e) = msg_handler.send_ping(&mut ws_stream).await {
                        error!("Failed to send ping: {}", e);
                    }
                }
            }
        }
    }

    println!("\n✓ Received first prices for all requested tokens");
    Ok(())
}

/// Run continuous price streaming
async fn run_continuous_mode(tokens: Option<&Vec<String>>, provider_type: ProviderType) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting WebSocket price streaming from {}", provider_type.as_str());

    let default_tokens = vec!["btc".to_string(), "eth".to_string(), "mina".to_string()];
    let tokens_list = tokens.cloned().unwrap_or(default_tokens);
    let symbols = providers::get_provider_symbols(provider_type, &tokens_list);

    println!("Streaming prices for: {} (provider: {})", symbols.join(", "), provider_type.as_str());
    println!("Press Ctrl+C to stop\n");

    match provider_type {
        ProviderType::Binance => {
            run_continuous_mode_binance(symbols).await
        }
        ProviderType::Bybit => {
            run_continuous_mode_bybit(symbols).await
        }
        _ => Err(format!("Provider {} not yet implemented for WebSocket", provider_type.as_str()).into()),
    }
}

async fn run_continuous_mode_binance(symbols: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    #[allow(unused_imports)]
    use crate::providers::binance::{handler, MessageHandler, Statistics};

    let mut client = BinanceWebSocket::new(symbols);
    let mut stats = Statistics::new();

    loop {
        let mut ws_stream = match client.connect_with_retry().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to establish connection: {}", e);
                continue;
            }
        };

        let mut msg_handler = MessageHandler::new();
        let result = process_messages_binance(&mut ws_stream, &mut msg_handler, &mut stats).await;

        match result {
            Ok(true) => {
                info!("Shutting down...");
                stats.print_summary();
                break;
            }
            Ok(false) => {
                warn!("Connection lost, attempting to reconnect...");
                stats.increment_errors();
            }
            Err(e) => {
                error!("Error processing messages: {}", e);
                stats.increment_errors();
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn run_continuous_mode_bybit(symbols: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    #[allow(unused_imports)]
    use crate::providers::bybit::{handler, websocket::MessageHandler, handler::Statistics};

    let mut client = BybitWebSocket::new(symbols);
    let mut stats = Statistics::new();

    loop {
        let mut ws_stream = match client.connect_with_retry().await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to establish connection: {}", e);
                continue;
            }
        };

        let mut msg_handler = MessageHandler::new();
        let result = process_messages_bybit(&mut ws_stream, &mut msg_handler, &mut stats).await;

        match result {
            Ok(true) => {
                info!("Shutting down...");
                stats.print_summary();
                break;
            }
            Ok(false) => {
                warn!("Connection lost, attempting to reconnect...");
                stats.increment_errors();
            }
            Err(e) => {
                error!("Error processing messages: {}", e);
                stats.increment_errors();
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

/// Fetch prices via REST API
async fn run_rest_mode(tokens: Option<&Vec<String>>, provider_type: ProviderType) -> Result<(), Box<dyn std::error::Error>> {
    info!("Fetching prices via REST API from {}...", provider_type.as_str());

    let default_tokens = vec!["btc".to_string(), "eth".to_string(), "mina".to_string()];
    let tokens_list = tokens.cloned().unwrap_or(default_tokens);
    let symbols = providers::get_provider_symbols(provider_type, &tokens_list);

    println!("Fetching 24hr ticker data for: {} (provider: {})\n", symbols.join(", "), provider_type.as_str());

    // Create REST client based on provider
    let client: Box<dyn ProviderRest> = match provider_type {
        ProviderType::Binance => Box::new(BinanceRestClient::new()),
        ProviderType::Bybit => Box::new(providers::bybit::BybitRestClient::new()),
        ProviderType::Coinbase => Box::new(providers::coinbase::CoinbaseRestClient::new()),
    };

    // Fetch ticker data
    let tickers = client.get_24hr_ticker(&symbols).await?;

    // Display results
    for ticker in &tickers {
        print_ticker_24hr(ticker);
        println!(); // Blank line between tickers
    }

    println!("✓ Successfully fetched {} ticker(s)", tickers.len());
    Ok(())
}

/// Fetch news articles for cryptocurrencies
async fn run_news_mode(tokens: Option<&Vec<String>>) -> Result<(), Box<dyn std::error::Error>> {
    use crate::news::NewsApiClient;

    info!("Fetching news articles...");

    // Get API key from environment
    let api_key = std::env::var("NEWS_API_KEY")
        .map_err(|_| "NEWS_API_KEY not found in environment. Make sure .env file exists.")?;

    // Get query terms (crypto names)
    let default_queries = vec!["bitcoin".to_string(), "ethereum".to_string(), "mina".to_string()];
    let queries = if let Some(tokens) = tokens {
        tokens
            .iter()
            .map(|token| {
                let lower = token.to_lowercase();
                match lower.as_str() {
                    "btc" => "bitcoin".to_string(),
                    "eth" => "ethereum".to_string(),
                    "mina" => "mina".to_string(),
                    _ => lower,
                }
            })
            .collect()
    } else {
        default_queries
    };

    println!("Fetching news for: {}\n", queries.join(", "));

    // Create news client
    let client = NewsApiClient::new(api_key);

    // Fetch news for each query
    for query in &queries {
        match client.get_news(query).await {
            Ok(response) => {
                if response.articles.is_empty() {
                    println!("No news articles found for {}", query);
                    println!();
                    continue;
                }

                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("{} News ({} articles)", query.to_uppercase(), response.total_results);
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

                // Display up to 5 most recent articles
                for (i, article) in response.articles.iter().take(5).enumerate() {
                    print_article(article, i + 1);
                }

                println!();
            }
            Err(e) => {
                error!("Failed to fetch news for {}: {}", query, e);
                println!("✗ Error fetching news for {}: {}\n", query, e);
            }
        }
    }

    println!("✓ News fetch complete");
    Ok(())
}

/// Format and print a news article
fn print_article(article: &crate::news::Article, index: usize) {
    use chrono::{DateTime, Utc};

    // Parse and format published date
    let published = DateTime::parse_from_rfc3339(&article.published_at)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| article.published_at.clone());

    // Calculate total text size (title + description + content)
    let mut text_size = article.title.len();
    if let Some(desc) = &article.description {
        text_size += desc.len();
    }
    if let Some(content) = &article.content {
        text_size += content.len();
    }

    println!("{}. {}", index, article.title);
    println!("   Source: {}", article.source.name);
    if let Some(author) = &article.author {
        println!("   Author: {}", author);
    }
    println!("   Published: {}", published);
    println!("   Text Size: {} bytes", text_size);
    println!();
    if let Some(description) = &article.description {
        println!("   Description:");
        println!("   {}", description);
        println!();
    }
    if let Some(content) = &article.content {
        println!("   Content:");
        println!("   {}", content);
        println!();
    }
    println!("   URL: {}", article.url);
    println!();
}

/// Format and print 24hr ticker data
fn print_ticker_24hr(ticker: &crate::provider::TickerData) {
    use chrono::{DateTime, Local, Utc};

    // Format symbol (e.g., "BTCUSDT" -> "BTC/USDT")
    let symbol = format_symbol_display(&ticker.symbol);

    // Parse times
    let open_time = DateTime::<Utc>::from_timestamp_millis(ticker.open_time)
        .map(|dt| dt.with_timezone(&Local))
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let close_time = DateTime::<Utc>::from_timestamp_millis(ticker.close_time)
        .map(|dt| dt.with_timezone(&Local))
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Calculate spread
    let bid: f64 = ticker.bid_price.parse().unwrap_or(0.0);
    let ask: f64 = ticker.ask_price.parse().unwrap_or(0.0);
    let spread = ask - bid;

    // Format change indicator
    let change_indicator = if ticker.price_change.starts_with('-') {
        ""
    } else {
        "+"
    };

    println!("{} - 24hr Ticker Statistics", symbol);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Price:          ${}", ticker.last_price);
    println!(
        "24h Change:     {}{} ({}{}%)",
        change_indicator, ticker.price_change, change_indicator, ticker.price_change_percent
    );
    println!("24h High:       ${}", ticker.high_price);
    println!("24h Low:        ${}", ticker.low_price);
    println!("24h Volume:     {} {}", ticker.volume, extract_base_asset(&ticker.symbol));
    println!("Quote Volume:   ${}", ticker.quote_volume);
    println!();
    println!("Best Bid:       ${}  (Qty: {})", ticker.bid_price, ticker.bid_qty);
    println!("Best Ask:       ${}  (Qty: {})", ticker.ask_price, ticker.ask_qty);
    println!("Spread:         ${:.8}", spread);
    println!();
    println!("Open Price:     ${}", ticker.open_price);
    println!("Close Price:    ${}", ticker.last_price);
    println!("Prev Close:     ${}", ticker.prev_close_price);
    println!("Trades:         {}", ticker.count);
    println!();
    println!("Period: {} → {}", open_time, close_time);
}

/// Extract base asset from symbol (e.g., "BTCUSDT" -> "BTC")
fn extract_base_asset(symbol: &str) -> String {
    let upper = symbol.to_uppercase();
    if upper.ends_with("USDT") {
        upper[..upper.len() - 4].to_string()
    } else if upper.ends_with("USDC") {
        upper[..upper.len() - 4].to_string()
    } else if upper.ends_with("BTC") {
        upper[..upper.len() - 3].to_string()
    } else {
        upper
    }
}

/// Format symbol for display (e.g., "BTCUSDT" -> "BTC/USDT")
fn format_symbol_display(symbol: &str) -> String {
    let upper = symbol.to_uppercase();
    if upper.ends_with("USDT") {
        let base = &upper[..upper.len() - 4];
        format!("{}/USDT", base)
    } else if upper.ends_with("USDC") {
        let base = &upper[..upper.len() - 4];
        format!("{}/USDC", base)
    } else if upper.ends_with("BTC") {
        let base = &upper[..upper.len() - 3];
        format!("{}/BTC", base)
    } else {
        upper
    }
}

/// Extract symbol from a Binance message
fn get_symbol_from_binance_message(message: &providers::binance::models::BinanceMessage) -> Option<String> {
    use providers::binance::models::BinanceMessage;
    match message {
        BinanceMessage::Trade(trade) => Some(trade.symbol.clone()),
        BinanceMessage::AggTrade(agg_trade) => Some(agg_trade.symbol.clone()),
        BinanceMessage::Ticker(ticker) => Some(ticker.symbol.clone()),
        _ => None,
    }
}

/// Extract symbol from a Bybit message
fn get_symbol_from_bybit_message(message: &providers::bybit::models::BybitMessage) -> Option<String> {
    use providers::bybit::models::BybitMessage;
    match message {
        BybitMessage::Trade(trades) => trades.first().map(|t| t.symbol.clone()),
        BybitMessage::Ticker(ticker) => Some(ticker.symbol.clone()),
        _ => None,
    }
}

async fn process_messages_binance(
    ws_stream: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    msg_handler: &mut providers::binance::websocket::MessageHandler,
    stats: &mut providers::binance::handler::Statistics,
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::providers::binance::handler;

    let mut shutdown = Box::pin(signal::ctrl_c());

    loop {
        tokio::select! {
            Some(msg_result) = ws_stream.next() => {
                match msg_result {
                    Ok(msg) => {
                        match msg_handler.process_message(msg).await {
                            Ok(Some(text)) => {
                                match handler::parse_message(&text) {
                                    Ok(binance_msg) => {
                                        handler::print_message(binance_msg.clone());
                                        stats.update(&binance_msg);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse message: {}", e);
                                        stats.increment_errors();
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("Message processing error: {}", e);
                                return Ok(false);
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        return Ok(false);
                    }
                }
            }
            _ = &mut shutdown => {
                info!("Received shutdown signal");
                return Ok(true);
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                if !msg_handler.is_healthy(Duration::from_secs(60)) {
                    warn!("No messages received for 60 seconds, reconnecting...");
                    return Ok(false);
                }
            }
        }
    }
}

async fn process_messages_bybit<S>(
    ws_stream: &mut S,
    msg_handler: &mut providers::bybit::websocket::MessageHandler,
    stats: &mut providers::bybit::handler::Statistics,
) -> Result<bool, Box<dyn std::error::Error>>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + futures_util::SinkExt<Message> + Unpin + Send,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    use crate::providers::bybit::handler;

    let mut shutdown = Box::pin(signal::ctrl_c());

    loop {
        tokio::select! {
            Some(msg_result) = ws_stream.next() => {
                // Check if we need to send ping
                if msg_handler.should_send_ping() {
                    if let Err(e) = msg_handler.send_ping(ws_stream).await {
                        error!("Failed to send ping: {}", e);
                    }
                }

                match msg_result {
                    Ok(msg) => {
                        match msg_handler.process_message(msg).await {
                            Ok(Some(text)) => {
                                match handler::parse_message(&text) {
                                    Ok(bybit_msg) => {
                                        handler::print_message(bybit_msg.clone());
                                        stats.update(&bybit_msg);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse message: {}", e);
                                        stats.increment_errors();
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("Message processing error: {}", e);
                                return Ok(false);
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        return Ok(false);
                    }
                }
            }
            _ = &mut shutdown => {
                info!("Received shutdown signal");
                return Ok(true);
            }
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                // Send ping
                if msg_handler.should_send_ping() {
                    if let Err(e) = msg_handler.send_ping(ws_stream).await {
                        error!("Failed to send ping: {}", e);
                    }
                }

                // Health check
                if !msg_handler.is_healthy(Duration::from_secs(60)) {
                    warn!("No messages received for 60 seconds, reconnecting...");
                    return Ok(false);
                }
            }
        }
    }
}

/// Calculate and display pivot points for cryptocurrencies
async fn run_pivot_mode(tokens: Option<&Vec<String>>, interval: &str, market: &str) -> Result<(), Box<dyn std::error::Error>> {
    use crate::pivot::calculate_all_pivots;
    use crate::providers::binance::{BinanceKlineClient, MarketType};

    info!("Calculating pivot points...");

    // Parse market type
    let market_type = match market.to_lowercase().as_str() {
        "spot" => MarketType::Spot,
        "futures" => MarketType::Futures,
        _ => {
            return Err(format!("Invalid market type '{}'. Use 'spot' or 'futures'.", market).into());
        }
    };

    let default_tokens = vec!["btc".to_string(), "eth".to_string(), "mina".to_string()];
    let tokens_list = tokens.cloned().unwrap_or(default_tokens);
    let symbols = providers::get_provider_symbols(provider::ProviderType::Binance, &tokens_list);

    println!("Calculating pivot points for: {} (interval: {}, market: {})\n", symbols.join(", "), interval, market);

    // Create kline client
    let client = BinanceKlineClient::new(market_type);

    // Calculate pivots for each symbol
    for symbol in &symbols {
        match client.get_latest_kline(symbol, interval).await {
            Ok(kline) => {
                // Parse OHLC data
                let (open, high, low, close) = match kline.to_ohlc() {
                    Ok(ohlc) => ohlc,
                    Err(e) => {
                        error!("Failed to parse kline data for {}: {}", symbol, e);
                        println!("✗ Error parsing data for {}: {}\n", symbol, e);
                        continue;
                    }
                };

                // Calculate all pivot points
                let pivots = calculate_all_pivots(open, high, low, close);

                // Display results
                print_pivot_points(symbol, interval, &pivots);
            }
            Err(e) => {
                error!("Failed to fetch kline data for {}: {}", symbol, e);
                println!("✗ Error fetching data for {}: {}\n", symbol, e);
            }
        }
    }

    println!("✓ Pivot points calculation complete");
    Ok(())
}

/// Format and print pivot points
fn print_pivot_points(symbol: &str, interval: &str, pivots: &pivot::PivotPoints) {
    let symbol_display = format_symbol_display(symbol);

    println!("{} - Pivot Points ({} interval)", symbol_display, interval);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Classic Pivots
    println!("Classic Pivots:");
    println!("  R3: {:>10.4}    S3: {:>10.4}", pivots.classic.r3, pivots.classic.s3);
    println!("  R2: {:>10.4}    S2: {:>10.4}", pivots.classic.r2, pivots.classic.s2);
    println!("  R1: {:>10.4}    S1: {:>10.4}", pivots.classic.r1, pivots.classic.s1);
    println!("   P: {:>10.4}", pivots.classic.pivot);
    println!();

    // Fibonacci Pivots
    println!("Fibonacci Pivots:");
    println!("  R3: {:>10.4}    S3: {:>10.4}", pivots.fibonacci.r3, pivots.fibonacci.s3);
    println!("  R2: {:>10.4}    S2: {:>10.4}", pivots.fibonacci.r2, pivots.fibonacci.s2);
    println!("  R1: {:>10.4}    S1: {:>10.4}", pivots.fibonacci.r1, pivots.fibonacci.s1);
    println!("   P: {:>10.4}", pivots.fibonacci.pivot);
    println!();

    // Camarilla Pivots
    println!("Camarilla Pivots:");
    println!("  R4: {:>10.4}    S4: {:>10.4}", pivots.camarilla.r4, pivots.camarilla.s4);
    println!("  R3: {:>10.4}    S3: {:>10.4}", pivots.camarilla.r3, pivots.camarilla.s3);
    println!("  R2: {:>10.4}    S2: {:>10.4}", pivots.camarilla.r2, pivots.camarilla.s2);
    println!("  R1: {:>10.4}    S1: {:>10.4}", pivots.camarilla.r1, pivots.camarilla.s1);
    println!();

    // Woodie Pivots
    println!("Woodie Pivots:");
    println!("  R2: {:>10.4}    S2: {:>10.4}", pivots.woodie.r2, pivots.woodie.s2);
    println!("  R1: {:>10.4}    S1: {:>10.4}", pivots.woodie.r1, pivots.woodie.s1);
    println!("   P: {:>10.4}", pivots.woodie.pivot);
    println!();

    // DeMark Pivots
    println!("DeMark Pivots:");
    println!("  R1: {:>10.4}    S1: {:>10.4}", pivots.demark.r1, pivots.demark.s1);
    println!("   P: {:>10.4}", pivots.demark.pivot);
    println!();
}

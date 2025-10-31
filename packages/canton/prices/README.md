# Binance WebSocket Price Streaming Application

A Rust application that connects to Binance WebSocket feeds to stream real-time cryptocurrency price data.

## Features

- Real-time price streaming for BTC/USDT, ETH/USDT, and MINA/USDT
- Displays both trade executions and 24hr ticker statistics
- Two modes: **get** (fetch first price and exit) and **run** (continuous streaming)
- Flexible token selection via CLI
- Automatic reconnection on disconnect
- Health monitoring with 60-second timeout
- Rate limiting compliance
- Graceful shutdown with Ctrl+C

## Usage

### Get First Prices (Quick Fetch)

Get the first available price for each token and exit:

```bash
# Get prices for all default tokens (BTC, ETH, MINA)
cargo run -- get

# Get prices for specific tokens
cargo run -- get --token btc,eth
cargo run -- get --token mina
```

### Run Continuous Streaming

Stream prices continuously:

```bash
# Stream all default tokens (BTC, ETH, MINA)
cargo run -- run

# Stream specific tokens
cargo run -- run --token btc
cargo run -- run --token btc,eth,mina
```

### Build

```bash
# Debug build
cargo build

# Optimized release build
cargo build --release
```

## Output Format

The application displays real-time updates in the following formats:

### Trade Data

```
[timestamp] SYMBOL TRADE: $price | Qty: quantity | Side: BUY/SELL | ID: trade_id
```

### Ticker Data (24hr Statistics)

```
[timestamp] SYMBOL TICKER: $current_price | 24h: +/-change% | High: $high | Low: $low | Vol: volume
```

## Architecture

- **main.rs**: Application entry point with async runtime and message processing loop
- **websocket.rs**: WebSocket connection management with automatic reconnection
- **models.rs**: Data structures for Binance message types
- **handler.rs**: Message parsing and formatting logic

## CLI Reference

```bash
# Show help
cargo run -- --help

# Show help for a specific command
cargo run -- get --help
cargo run -- run --help
```

### Commands

- `get` - Get first prices for each token and exit
- `run` - Run continuous price streaming

### Options

- `--token <TOKENS>` - Comma-separated list of tokens (btc, eth, mina). Defaults to all three if not specified.

## Note on News Feeds

Binance WebSocket API does not provide news/announcement streams. For news integration, you would need to use third-party news APIs or poll REST endpoints.

## Dependencies

- tokio: Async runtime
- tokio-tungstenite: WebSocket client with TLS support
- serde/serde_json: JSON deserialization
- futures-util: Stream utilities
- tracing: Logging
- chrono: Timestamp formatting
- clap: Command-line argument parsing

# Using Witness as a Library

The `witness` package is now both a CLI binary and a library that can be used by other Rust crates.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
witness = { path = "../witness" }
tokio = { version = "1.40", features = ["full"] }
```

## Usage Examples

### Fetch Complete Price Proof Data

```rust
use witness::fetch_price_proof_data;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Fetch all proof data: price, TLS certs, Sui checkpoint, TSA timestamp
    // Pass the trading pair symbol (e.g., "BTCUSDT", "ETHUSDT", "SOLUSDT")
    let proof_data = fetch_price_proof_data("BTCUSDT").await?;

    println!("BTC Price: ${}", proof_data.price.price);
    println!("Verified with {} certificates", proof_data.certificates.certificates_der.len());
    println!("TSA timestamp: {}", proof_data.tsa_timestamp.time_string);

    Ok(())
}
```

### Use Individual Modules

```rust
use witness::{binance, sui, tsa};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Fetch just the price from Binance with TLS verification
    let (price, certs) = binance::fetch_and_verify_price("BTCUSDT").await?;

    // Get latest Sui checkpoint
    let checkpoint = sui::get_last_checkpoint().await?;

    // Get TSA timestamp for any data
    let data = b"Hello, World!";
    let timestamp = tsa::get_timestamp(data, "http://timestamp.digicert.com").await?;

    Ok(())
}
```

### Access Submodules Directly

```rust
// All modules are public
use witness::binance;
use witness::sui;
use witness::tsa;
use witness::price_proof;

// Individual functions from each module
use witness::{
    fetch_price_proof_data,
    fetch_and_verify_price,
    get_last_checkpoint,
    get_timestamp,
};
```

## Exported Functions

### Top-level exports
- `fetch_price_proof_data(symbol: &str) -> Result<PriceProofData>` - Fetch complete proof with all attestations for a trading pair

### Module: `witness::binance`
- `fetch_price(symbol: &str) -> Result<PriceData>` - Fetch price from Binance API
- `verify_binance_certificate() -> Result<CertificateChain>` - Verify Binance TLS certificate chain
- `fetch_and_verify_price(symbol: &str) -> Result<(PriceData, CertificateChain)>` - Fetch price + verify certs

### Module: `witness::sui`
- `get_last_checkpoint() -> Result<CheckpointInfo>` - Get latest Sui mainnet checkpoint

### Module: `witness::tsa`
- `get_timestamp(data: &[u8], endpoint: &str) -> Result<TsaResponse>` - Get TSA timestamp with certificate verification

## CLI Binary

The CLI binary is still available:

```bash
# Fetch proof for BTC (default)
cargo run --bin witness proof

# Fetch proof for ETH
cargo run --bin witness proof --token ETH

# Fetch proof for SOL
cargo run --bin witness proof -t SOL
```

## Integration with zkVM

The witness library fetches data with network I/O (async operations). For zkVM execution:

1. Use `witness` library to fetch proof data outside the zkVM
2. Use `price-lib` library inside the zkVM to verify the proof (no async/network)

```rust
// Outside zkVM (host/prover)
let proof_data = witness::fetch_price_proof_data("BTCUSDT").await?;

// Inside zkVM (guest program)
use price_lib::verify_proof_data;
let verification = verify_proof_data(&proof_data)?;
```

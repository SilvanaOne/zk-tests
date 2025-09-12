# Scan - Canton Ledger CLI

A Rust-based gRPC client for querying Canton/Daml ledger transactions and balances.

## Features

- 🔍 Query ledger transactions via gRPC
- 💰 Check wallet balances
- 📄 View active contracts
- ⏰ Filter transactions by time (last hour)
- ⚙️ Configurable via environment variables or `.env` file
- 🔐 JWT authentication support
- 🎨 Colored terminal output with tables

## Installation

```bash
# Build the project
cargo build --release

# The binary will be at target/release/scan
```

## Configuration

Create a `.env` file based on `.env.example`:

```bash
cp .env.example .env
# Edit .env with your configuration
```

### Environment Variables

- `LEDGER_HOST`: Canton ledger host (default: localhost)
- `LEDGER_PORT`: Canton ledger port (default: 3901)
- `VALIDATOR_PORT`: Validator API port (default: 3903)
- `JWT_SECRET`: JWT signing secret (default: unsafe)
- `JWT_AUDIENCE`: JWT audience (default: https://canton.network.global)
- `JWT_USER`: JWT user (default: ledger-api-user)
- `PARTY_ID`: Party identifier for queries
- `USE_TLS`: Enable TLS connection (default: false)

### Port Configuration

- **app-user**: ledger port 2901, validator port 2903
- **app-provider**: ledger port 3901, validator port 3903
- **sv**: ledger port 4901, validator port 4903

## Usage

```bash
# Show help
scan --help

# Show configuration
scan config

# Show ledger status
scan status

# Show wallet balance
scan balance

# Show all transactions
scan transactions

# Show transactions with limit
scan transactions --limit 50

# Show transactions from last hour
scan hour

# Show active contracts
scan contracts

# Show all information
scan all

# Override configuration via CLI
scan --host localhost --port 2901 balance

# Use different party
scan --party "app_user_localnet-localparty-1::..." transactions
```

## Commands

| Command        | Description                          |
| -------------- | ------------------------------------ |
| `balance`      | Show wallet balance (AMT amounts)    |
| `transactions` | List all transactions with details   |
| `hour`         | Show transactions from the last hour |
| `contracts`    | Display active contracts             |
| `config`       | Show current configuration           |
| `status`       | Display ledger version and status    |
| `all`          | Show all information combined        |

## Examples

```bash
# Check app-provider balance
LEDGER_PORT=3901 scan balance

# View app-user transactions
LEDGER_PORT=2901 PARTY_ID="app_user_localnet-localparty-1::..." scan transactions

# Show last hour's transactions for sv
LEDGER_PORT=4901 scan hour
```

## Development

```bash
# Run in development mode
cargo run -- status

# Run with logging
RUST_LOG=debug cargo run -- transactions

# Format code
cargo fmt

# Run clippy
cargo clippy
```

## Architecture

The CLI uses:

- **tonic** for gRPC communication
- **prost** for protobuf serialization
- **tokio** for async runtime
- **clap** for CLI argument parsing
- **jsonwebtoken** for JWT generation
- **tabled** for formatted output

## Proto Files

The proto files are copied from the Canton ledger API and compiled at build time using `tonic-build`.

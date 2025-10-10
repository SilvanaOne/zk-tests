# Canton Billing System

A comprehensive Rust-based billing and subscription management system for Canton blockchain applications, featuring automated payment processing, metrics tracking, and real-time monitoring.

## Features

### 🔐 **Blockchain Integration**

- **Canton Network Support**: Native integration with Canton blockchain for payment processing
- **TransferPreapproval Management**: Automated setup and management of pre-approved payment transfers
- **OpenMiningRound Selection**: Intelligent selection of the earliest valid mining round for transactions

### 💳 **Payment Processing**

- **Automated Recurring Payments**: Process subscription payments automatically based on billing intervals
- **Payment Retry Logic**: Automatic retry mechanism for failed payments with exponential backoff
- **Transaction Validation**: Real-time validation of payment transactions on the blockchain

### 📊 **Metrics & Analytics**

- **Multi-Window Aggregation**: Track metrics across 7 time windows (10m, 1h, 6h, 12h, 24h, 7d, 30d)
- **Comprehensive Payment Metrics**:
  - Payment counts and success rates
  - Total payment amounts
  - Active users and subscriptions
  - User-specific and subscription-specific breakdowns
- **RocksDB Persistence**: High-performance local storage for metrics with configurable retention
- **Real-time Aggregation**: Automatic metric aggregation running every 60 seconds

### 📡 **Monitoring & Observability**

- **OpenTelemetry Integration**: Full support for distributed tracing and metrics export
- **BetterStack Support**: Direct integration with BetterStack for cloud monitoring
- **Structured Logging**: Comprehensive tracing with configurable log levels
- **Dashboard Support**: Pre-configured BetterStack dashboard for visualizing all metrics

### 👥 **User & Subscription Management**

- **JSON-Based Configuration**: Load users and subscriptions from JSON files
- **Flexible Subscription Plans**: Support for multiple subscription tiers with custom features
- **Active Subscription Tracking**: Real-time tracking of subscription status and expiration
- **User Querying**: Search and filter users by various criteria

## Architecture

### Core Components

1. **Payment Engine** (`pay.rs`)

   - Handles blockchain transaction creation and submission
   - Manages payment lifecycle from initiation to confirmation
   - Implements retry logic and error handling

2. **Metrics System** (`metrics.rs`)

   - Tracks payment events in real-time
   - Aggregates data across multiple time windows
   - Provides querying interface for analytics

3. **Database Layer** (`db.rs`)

   - RocksDB integration for persistent storage
   - Efficient key-value operations for metrics
   - Configurable retention policies

4. **Monitoring Integration** (`monitoring.rs`)

   - OpenTelemetry exporter for metrics and logs
   - BetterStack integration for cloud monitoring
   - Custom tracing layer for structured logging

5. **Recovery System** (`recovery.rs`)
   - Calculates missed payments during downtime
   - Manages payment retry queue
   - Ensures payment continuity

## Configuration

### Environment Variables

```bash
# Canton Network Configuration
CANTON_CHAIN=localnet
APP_NAME=YourAppName
PARTY_APP=app1::1220...  # Canton party ID with payment amulets

# API Endpoints
APP_PROVIDER_API_URL=http://localhost:3975/
SCAN_API_URL=http://scan.localhost:4000/api/scan/

# Authentication
APP_PROVIDER_JWT=eyJhbG...  # JWT token for API access

# OpenTelemetry (Optional)
OPENTELEMETRY_INGESTING_HOST=xxxxx.betterstackdata.com
OPENTELEMETRY_SOURCE_ID=canton_billing
OPENTELEMETRY_SOURCE_TOKEN=2Y1xxxxx

# Database
RETENTION_DAYS=365  # Metrics retention period

# Logging
RUST_LOG=info  # Log level (trace, debug, info, warn, error)
```

### Data Files

- `data/users.json` - User definitions and subscription assignments
- `data/subscriptions.json` - Available subscription plans and pricing
- `dashboard/billing.json` - BetterStack dashboard configuration

## Metrics Exported

The system exports the following metrics to OpenTelemetry:

### Per Time Window Metrics

For each time window (10m, 1h, 6h, 12h, 24h, 7d, 30d):

- `canton.billing.window.{window}.payment_count` - Number of payments
- `canton.billing.window.{window}.total_amount` - Total payment amount
- `canton.billing.window.{window}.success_rate` - Payment success rate
- `canton.billing.window.{window}.active_users` - Number of active users
- `canton.billing.window.{window}.active_subscriptions` - Number of active subscriptions

### Attributes

Each metric includes attributes for:

- `user` - Canton party ID of the user
- `subscription` - Subscription plan identifier
- `window` - Time window identifier

## CLI Commands

The Canton Billing CLI provides comprehensive commands for managing subscriptions, payments, users, and monitoring on the Canton blockchain.

### Command Overview

```bash
cargo run -- [COMMAND] [OPTIONS]

# Or after building:
./target/release/billing [COMMAND] [OPTIONS]
```

### Available Commands

#### `subscriptions` - List Available Subscriptions
Lists all configured subscription plans with pricing and features.

```bash
cargo run -- subscriptions
```

Output shows subscription names, IDs, prices, billing intervals, and features.

#### `users` - User Management
Manage and query user information.

**Subcommands:**
- `list` - List all users with their subscriptions
- `with-subscription <name>` - List users with a specific subscription

```bash
# List all users
cargo run -- users list

# Find users with "premium" subscription
cargo run -- users with-subscription premium
```

#### `user` - Find Specific User
Search for a user by email, name, or party ID substring.

```bash
# Find user by email
cargo run -- user alice@example.com

# Find user by name
cargo run -- user alice

# Find user by party ID substring
cargo run -- user "1220aca50"
```

#### `balance` - Check Canton Credit Balance
Display Canton Credit (CC) balance for a party.

```bash
# Check balance for PARTY_APP (default)
cargo run -- balance

# Check balance for specific party
cargo run -- balance --party "userparty1::1220aca50c19712a4247e9b74ab680b358962ae97f50c01577b92d03b2ae7dc83b10"
```

Output shows:
- Individual Amulet contracts with amounts
- Round numbers
- Total balance summary

#### `pay` - Execute Single Payment
Process a payment for a specific user and subscription.

```bash
# Process payment for user "alice" with "premium" subscription
cargo run -- pay --user alice --subscription premium

# Dry run mode (simulate without executing)
cargo run -- pay --user alice --subscription premium --dry-run
```

#### `start` - Automated Payment Processing
Start automated payment processing for all active subscriptions.

```bash
# Run continuous payment scheduler (default: 60 second intervals)
cargo run -- start

# Run once only
cargo run -- start --once

# Custom interval (300 seconds)
cargo run -- start --interval 300

# Dry run mode
cargo run -- start --dry-run
```

#### `setup` - Initialize TransferPreapproval
Setup TransferPreapproval contract for automated payments.

```bash
# Setup with 1 year expiration (default)
cargo run -- setup

# Setup with custom expiration (30 days = 43200 minutes)
cargo run -- setup --expires-in-min 43200
```

#### `restart` - Recovery and Restart
Restart payment processing and recover missed payments during downtime.

```bash
# Full restart (process pending and recover missed)
cargo run -- restart

# Process only pending payments
cargo run -- restart --process-pending --no-recover-missed

# Limit number of payments to process
cargo run -- restart --limit 50

# Dry run mode
cargo run -- restart --dry-run
```

#### `metrics` - Payment Analytics
Display payment metrics for various time windows.

```bash
# Overall metrics for last hour
cargo run -- metrics --window 1h

# Metrics for last 24 hours
cargo run -- metrics --window 24h

# User-specific metrics
cargo run -- metrics --window 1h --user alice

# Subscription-specific metrics
cargo run -- metrics --window 7d --subscription premium

# Combined user and subscription metrics
cargo run -- metrics --window 30d --user alice --subscription premium
```

Time windows: `10m`, `1h`, `6h`, `12h`, `24h`, `7d`, `30d`

#### `update` - Transaction Details
Get detailed information about a specific transaction update.

```bash
# Get update details by ID
cargo run -- update "update-id-12345"
```

### Global Options

#### `--log-level` - Logging Verbosity
Control the logging output level.

```bash
# Set debug logging
cargo run -- --log-level debug balance

# Set error-only logging
cargo run -- --log-level error start

# Levels: trace, debug, info (default), warn, error
```

### Usage Examples

#### Initial Setup
```bash
# Set up environment
cp .env.example .env
# Edit .env with your configuration

# Initialize TransferPreapproval for 1 year
cargo run -- setup --expires-in-min 525600

# Verify balances
cargo run -- balance
```

#### Daily Operations
```bash
# Check user balances
cargo run -- balance --party "userparty1::..."

# Process single payment
cargo run -- pay --user alice --subscription premium

# Start automated processing
cargo run -- start

# Check metrics
cargo run -- metrics --window 24h
```

#### Monitoring and Debugging
```bash
# Enable debug logging for troubleshooting
cargo run -- --log-level debug start --dry-run

# Check specific user status
cargo run -- user alice

# View payment metrics for specific subscription
cargo run -- metrics --window 1h --subscription premium

# Get transaction details
cargo run -- update "txn-update-id"
```

#### Recovery After Downtime
```bash
# Check pending payments
cargo run -- restart --dry-run

# Process all pending and missed payments
cargo run -- restart --limit 100

# Verify recovery with metrics
cargo run -- metrics --window 1h
```

### Testing

```bash
# Run unit tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- pay-all

# Test OpenTelemetry integration
cargo run -- test-otel
```

## Dashboard Setup

1. Import `dashboard/billing.json` to BetterStack
2. Configure the source ID in the dashboard to match your `OPENTELEMETRY_SOURCE_ID`
3. Access real-time metrics and visualizations

## Performance

- **Payment Processing**: ~1-2 seconds per transaction
- **Metric Aggregation**: <100ms for all windows
- **Database Operations**: <1ms for reads, <10ms for writes
- **Memory Usage**: ~50-100MB baseline
- **Storage**: ~1KB per payment event

## Security Considerations

- Store JWT tokens securely and rotate regularly
- Use environment variables for sensitive configuration
- Implement rate limiting for payment operations
- Monitor failed payment attempts for suspicious activity
- Regularly audit TransferPreapproval configurations

## License

[Your License Here]

## Contributing

[Contribution Guidelines]

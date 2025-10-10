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

## Usage Examples

### Initial Setup

```bash
# Set up environment
cp .env.example .env
# Edit .env with your configuration

# Initialize TransferPreapproval
cargo run -- setup --expires-in-min 525600  # 1 year expiration

# Verify configuration
cargo run -- info
```

### Process Payments

```bash
# Process single payment
cargo run -- pay --user alice --subscription premium

# Process all active subscriptions
cargo run -- pay-all

# Restart failed payments
cargo run -- restart --limit 10
```

### Monitor System

```bash
# Start monitoring with OpenTelemetry export
cargo run -- monitor

# View metrics for last hour
cargo run -- metrics --window 1h

# List active users
cargo run -- users
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

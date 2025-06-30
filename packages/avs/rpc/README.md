# Silvana RPC

A gRPC service that accepts event streams and efficiently batches them to TiDB serverless and NATS JetStream.

## Features

- **gRPC Event Streaming**: High-performance event ingestion with protobuf
- **Multi-Coordinator Support**: All events include `coordinator_id` for tracking events per coordinator
- **Event Buffering**: Configurable in-memory buffering with automatic batch processing
- **TiDB Integration**: Efficient batch insertion to TiDB serverless with transaction support
- **NATS JetStream**: Event streaming to NATS for real-time processing (placeholder implementation)
- **Type Safety**: Sea-ORM entities for all event types with compile-time validation
- **Monitoring**: Built-in statistics and performance tracking
- **Production Ready**: Configurable batch sizes, timeouts, and connection pooling
- **Memory Safety**: Bounded channels, circuit breakers, and backpressure to prevent OOM

## Quick Start

1. **Install Dependencies**

   ```bash
   # Install Rust and Cargo
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone and Build**

   ```bash
   git clone <repository>
   cd rpc
   cargo build --release
   ```

3. **Configure Environment**

   Create a `.env` file (loaded automatically with `dotenvy`):

   ```bash
   # Database Configuration
   DATABASE_URL=mysql://username:password@gateway01.ap-southeast-1.prod.aws.tidbcloud.com:4000/silvana_events?ssl-mode=REQUIRED

   # Server Configuration
   SERVER_ADDR=0.0.0.0:50051

   # Event Processing
   BATCH_SIZE=100              # Minimum trigger size (adaptive batching)
   FLUSH_INTERVAL_SECS=1       # Max seconds between flushes
   CHANNEL_CAPACITY=10000      # Max events in memory

   # Logging
   RUST_LOG=info
   ```

4. **Run Migrations**

   ```bash
   # Install sea-orm-cli for migrations
   cargo install sea-orm-cli

   # Run database migrations
   sea-orm-cli migrate up
   ```

5. **Reset Database (Optional)**

   If you need to reset your database (drop all tables and recreate them):

   ```bash
   # Make sure your DATABASE_URL is set in .env or environment
   ./reset_database.sh
   ```

   This script will:

   - **Drop ALL existing tables** in the database (complete clean slate)
   - Reset the migration state
   - Recreate all 9 RPC event tables with fresh schema
   - Verify table creation and show summary

   **Prerequisites for reset script:**

   - `sea-orm-cli` installed: `cargo install sea-orm-cli`
   - `mysql` client installed (optional, for verification)
   - `DATABASE_URL` configured in `.env` or environment

6. **Start the Server**

   ```bash
   cargo run --release
   ```

7. **Test the Server** (Optional)

   ```bash
   # In another terminal, run integration tests
   cargo test --release --test integration_test -- --nocapture
   ```

   See **[TESTING.md](TESTING.md)** for detailed testing instructions.

## Database Schema

Each event type is stored in its own table with the following common fields:

- `id`: Primary key
- `coordinator_id`: Indexed string field for coordinator identification
- `timestamp`: Indexed event timestamp
- `event_data`: Formatted event details
- `created_at`: Record creation timestamp

This schema allows for efficient querying by coordinator and time ranges.

## Development

### Project Structure

```
├── proto/events.proto              # Protobuf definitions
├── src/
│   ├── main.rs                     # gRPC service implementation
│   ├── database.rs                 # TiDB integration
│   ├── buffer.rs                   # Event buffering system
│   └── entities/                   # Sea-ORM entity definitions
├── migration/                      # Database migration files
└── build.rs                       # Code generation
```

### Building

```bash
cargo build
```

### Database Management

**Run Migrations:**

```bash
sea-orm-cli migrate up -d ./migration
```

**Reset Database (Clean Slate):**

```bash
./reset_database.sh
```

**Check Migration Status:**

```bash
sea-orm-cli migrate status -d ./migration
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests (requires running database)
cargo test --test integration_test -- --nocapture
```

## Troubleshooting

### Database Issues

**Migration Errors:**

```bash
# Reset database and run fresh migrations (drops ALL tables!)
./reset_database.sh
```

**Connection Issues:**

```bash
# Verify DATABASE_URL format
echo $DATABASE_URL

# Test connection manually
mysql -h <host> -P <port> -u <user> -p<password> <database> -e "SHOW TABLES;"
```

**Table Schema Issues:**

```bash
# View current tables
sea-orm-cli migrate status -d ./migration

# Reset if needed
./reset_database.sh
```

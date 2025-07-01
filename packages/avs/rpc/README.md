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

3. **Test**

```sh
cargo test --release --test integration_test -- --nocapture
cargo test --test sequence_test -- --nocapture
```

## Development

### Project Structure

```
├── proto/events.proto              # Protobuf definitions
├── src/
│   ├── main.rs                     # gRPC service implementation
│   ├── database.rs                 # TiDB integration
│   ├── buffer.rs                   # Event buffering system
│   └── entities/                   # Sea-ORM entity definitions
├── tidb/                           # TiDB-related tools and data
│   ├── migration/                  # Database migration files
│   ├── proto-to-ddl/               # Proto to DDL converter tool
│   └── sql/                        # Generated SQL files
└── build.rs                       # Code generation
```

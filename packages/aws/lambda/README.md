# AWS Lambda Rust Calculator with Sui Blockchain Integration

A serverless calculator API built with Rust, deployed on AWS Lambda, with optional Sui blockchain integration for mathematical operations, secure keypair management, distributed key locking, Silvana registry creation, and automated backup capabilities.

## Architecture

- **AWS Lambda**: Serverless compute using ARM64 architecture with custom Rust runtime
- **API Gateway**: REST API endpoint for HTTP access
- **OpenAPI**: API-first development with automated code generation
- **Sui Blockchain**: On-chain computation for addition operations and registry management
- **DynamoDB**: Distributed locking for Sui keys and secure storage for encrypted keypairs
- **KMS**: Encryption at rest for private keys
- **AWS Backup**: Automated backup and recovery for DynamoDB tables
- **Pulumi**: Infrastructure as Code for AWS resource management

## Prerequisites

### Required Tools

1. **Rust** (latest stable)

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Cargo Lambda** (for building Lambda functions)

   ```bash
   cargo install cargo-lambda
   ```

3. **Node.js & npm** (for Pulumi)

   ```bash
   # macOS
   brew install node

   # or download from https://nodejs.org/
   ```

4. **Pulumi** (for infrastructure deployment)

   ```bash
   curl -fsSL https://get.pulumi.com | sh
   ```

5. **OpenAPI Generator** (for code generation)

   ```bash
   # macOS
   brew install openapi-generator
   ```

6. **AWS CLI** (configured with credentials)

   ```bash
   # macOS
   brew install awscli

   # Configure
   aws configure
   ```

7. **Python 3** (for JSON formatting in Makefile tests)

   ```bash
   # macOS (usually pre-installed)
   python3 --version

   # or install via Homebrew
   brew install python3
   ```

### Optional Tools

- **jq** (alternative JSON formatting tool)
- **curl** (for API testing, usually pre-installed)

## Project Structure

```
├── api/
│   └── openapi.yaml           # OpenAPI specification
├── crates/
│   ├── api/                   # API business logic
│   ├── api-generated/         # Auto-generated OpenAPI models
│   ├── db/                    # DynamoDB operations and secure storage
│   ├── kms/                   # KMS encryption/decryption
│   ├── lambda/                # Lambda handler
│   └── sui/                   # Sui blockchain integration
├── pulumi/
│   ├── index.ts               # Infrastructure definition
│   └── package.json           # Pulumi dependencies
└── Makefile                   # Build and deployment commands
```

## Quick Start

### 1. Clone and Setup

```bash
git clone <repository-url>
cd packages/aws/lambda

# Install Pulumi dependencies
cd pulumi && npm install && cd ..
```

### 2. Configure Pulumi

```bash
# Login to Pulumi (use local backend or Pulumi Cloud)
pulumi login --local
# or
pulumi login

# Set passphrase for encryption
export PULUMI_CONFIG_PASSPHRASE=<your-passphrase>

# Create a new stack
cd pulumi
pulumi stack init dev
cd ..
```

### 3. Configure AWS

```bash
# Set AWS credentials
export AWS_ACCESS_KEY_ID=<your-access-key>
export AWS_SECRET_ACCESS_KEY=<your-secret-key>
export AWS_REGION=us-east-1

# Or use AWS CLI profile
aws configure
```

### 4. Configure Sui Blockchain (Optional)

If you want to use blockchain integration:

```bash
# Create a .env file with your Sui credentials
cat > ../../sui/rpc-tx/.env << EOF
SUI_PACKAGE_ID=0x...
SUI_CHAIN=devnet
SUI_ADDRESS=0x...
SUI_SECRET_KEY=suiprivkey...
EOF

# Add secrets to Pulumi
make add-secrets
```

### 5. Build and Deploy

```bash
# Build Lambda function
make lambda

# Deploy to AWS
make deploy

# Or do both in one command
make deploy  # (automatically builds before deploying)
```

### 6. Test the API

```bash
# Test addition endpoint
make test

# Test with custom values
curl -X POST $(pulumi stack output functionUrl)/add \
  -H "Content-Type: application/json" \
  -d '{"a": 10, "b": 20}'
```

## Crate Organization

The project follows a modular architecture with specialized crates:

### Core Crates

- **`api`**: Business logic and request handlers
  - Async/sync wrappers for Lambda compatibility
  - Error handling and response formatting
  - Integration with blockchain and storage layers

- **`sui`**: Sui blockchain client (refactored modular structure)
  - `chain.rs`: Shared RPC URL configuration and helper functions
  - `add.rs`: Addition operation on blockchain
  - `registry.rs`: Silvana registry creation with event extraction
  - `client.rs`: Re-exports for backward compatibility
  - `keypair.rs`: Ed25519 keypair generation with Bech32 encoding

- **`db`**: DynamoDB operations
  - `lock.rs`: Distributed key locking mechanism
  - `secure_storage.rs`: Encrypted keypair storage
  - Shared DynamoDB client with `OnceLock` pattern

- **`kms`**: AWS KMS integration
  - AES-256-GCM encryption/decryption
  - Envelope encryption with data keys
  - Secure key material handling

- **`lambda`**: AWS Lambda handler
  - Request routing and response handling
  - Tracing and logging configuration
  - Error response formatting

## Development Workflow

### API Development Flow

1. **Define API**: Edit `api/openapi.yaml` with your API specification

2. **Generate Code**: Generate Rust models from OpenAPI spec

   ```bash
   make openapi-generate
   ```

3. **Implement Handler**: Add business logic in `crates/api/src/lib.rs`

   - Generated models are in `api_generated::models`
   - Implement handlers for each operation

4. **Update Lambda**: Route requests in `crates/lambda/src/handler.rs`

5. **Test Locally**: Build and test

   ```bash
   make lambda
   ```

6. **Deploy**: Push to AWS
   ```bash
   make deploy
   ```

### Adding New Endpoints

1. Update `api/openapi.yaml`:

   ```yaml
   paths:
     /your-endpoint:
       post:
         operationId: yourOperation
         requestBody:
           $ref: "#/components/schemas/YourRequest"
         responses:
           "200":
             $ref: "#/components/schemas/YourResponse"
   ```

2. Regenerate models:

   ```bash
   make openapi-generate
   ```

3. Implement handler in `crates/api/src/lib.rs`:

   ```rust
   pub fn your_operation(request: YourRequest) -> Result<YourResponse, ApiError> {
       // Your logic here
   }
   ```

4. Add routing in `process_request_async()`:
   ```rust
   match path {
       "/your-endpoint" => {
           let request: YourRequest = serde_json::from_str(body)?;
           let response = your_operation(request)?;
           Ok(serde_json::to_string(&response)?)
       }
       // ... other endpoints
   }
   ```

## Makefile Commands

```bash
make help            # Show all available commands
make build           # Build all crates
make lambda          # Build Lambda deployment package
make deploy          # Deploy to AWS (builds first)
make preview         # Preview infrastructure changes
make test            # Test the deployed API
make openapi-validate # Validate OpenAPI specification
make openapi-generate # Generate Rust code from OpenAPI
make add-secrets     # Add Sui secrets to Pulumi
make list-secrets    # List configured secrets
make remove-secrets  # Remove Sui secrets
```

## Configuration

### Environment Variables

The Lambda function uses these environment variables (set via Pulumi):

- `RUST_BACKTRACE`: Error tracing (default: "1")
- `LOG_LEVEL`: Logging level (default: "debug")
- `SUI_PACKAGE_ID`: Sui smart contract address for math operations (optional)
- `SUI_CHAIN`: Sui network (mainnet/testnet/devnet)
- `SUI_ADDRESS`: Sui wallet address
- `SUI_SECRET_KEY`: Sui private key (stored as Pulumi secret)
- `SILVANA_REGISTRY_PACKAGE`: Registry package ID for devnet
- `SILVANA_REGISTRY_PACKAGE_DEVNET`: Alternative registry package for devnet
- `SILVANA_REGISTRY_PACKAGE_TESTNET`: Registry package for testnet
- `SILVANA_REGISTRY_PACKAGE_MAINNET`: Registry package for mainnet
- `LOCKS_TABLE_NAME`: DynamoDB table for key locking (default: "sui-key-locks")
- `KEYPAIRS_TABLE_NAME`: DynamoDB table for encrypted keypairs (default: "sui-keypairs")
- `KMS_KEY_ID`: KMS key ID for encryption

### Infrastructure Settings

Edit `pulumi/index.ts` to configure:

- Lambda memory size (default: 512MB)
- Lambda timeout (default: 30s)
- Architecture (default: ARM64)
- Log retention (default: 7 days)

## Key Features

### Automated Backup and Recovery

The system implements comprehensive backup strategies for DynamoDB tables to ensure data durability and recovery capabilities:

#### Point-in-Time Recovery (PITR)
- **Continuous backups**: Automatically enabled for both keypairs and locks tables
- **35-day retention**: Can restore to any second within the past 35 days
- **Per-second granularity**: Precise recovery to any point in time
- **Zero data loss**: Ensures critical keypair data is never lost

#### AWS Backup Integration
- **Scheduled backups**: Three-tier backup strategy
  - **Daily backups**: Retained for 35 days
  - **Weekly backups**: Retained for 120 days (moved to cold storage after 30 days)
  - **Monthly backups**: Retained for 365 days (moved to cold storage after 90 days)
- **Cost optimization**: Automatic transition to cold storage for older backups
- **Backup vault**: Secure storage in `silvana-dynamodb-backups` vault
- **Automated scheduling**: No manual intervention required

#### Recovery Procedures
To restore from a backup:

1. **Point-in-time recovery** (for recent data):
   ```bash
   aws dynamodb restore-table-to-point-in-time \
     --source-table-name sui-keypairs \
     --target-table-name sui-keypairs-restored \
     --restore-date-time 2025-08-16T12:00:00Z
   ```

2. **Restore from backup vault** (for older snapshots):
   ```bash
   aws backup start-restore-job \
     --recovery-point-arn <recovery-point-arn> \
     --iam-role-arn <backup-role-arn> \
     --metadata '{"newTableName":"sui-keypairs-restored"}'
   ```

### Distributed Key Locking

The system implements a distributed locking mechanism using DynamoDB to prevent concurrent use of Sui private keys across multiple Lambda instances. This prevents transaction failures and key lockouts that can occur when the same key is used simultaneously.

- **Automatic lock acquisition**: Before executing Sui transactions
- **1-minute timeout**: Locks automatically expire after 60 seconds
- **Retry logic**: Handles expired locks and contention
- **Performance tracking**: Logs lock acquisition and hold times in milliseconds

### Secure Keypair Storage

Private keys are securely stored using AWS KMS encryption:

- **KMS envelope encryption**: Data keys are generated per encryption operation
- **AES-256-GCM**: Symmetric encryption for private key data
- **Composite keys**: Keypairs indexed by login_type + login identifier
- **Automatic key generation**: New keypairs created on first request
- **Key reuse**: Existing keypairs retrieved for returning users

## API Endpoints

### POST /add

Adds two numbers. Uses Sui blockchain if configured.

Request:

```json
{
  "a": 10,
  "b": 20
}
```

Response:

```json
{
  "result": 30,
  "operation": "add",
  "tx_hash": "0x..." // Only when using blockchain
}
```

### POST /multiply

Multiplies two numbers (local computation only).

Request:

```json
{
  "a": 10,
  "b": 20
}
```

Response:

```json
{
  "result": 200,
  "operation": "multiply"
}
```

### POST /generate-sui-keypair

Generates or retrieves a Sui Ed25519 keypair for a specific login. Private keys are encrypted and stored securely using AWS KMS.

Request:

```json
{
  "login_type": "google",
  "login": "user@example.com"
}
```

Response:

```json
{
  "address": "0x41d26f8218ba28e6ef35d58ddc937fc2e52706c2d0791cf1b3a03b229153f688"
}
```

**Note**: Private keys are stored encrypted in DynamoDB and are not returned to the client for security.

### POST /create-registry

Creates a new Silvana registry on the Sui blockchain.

Request:

```json
{
  "name": "My Registry",
  "chain": "devnet"  // Options: devnet, testnet, mainnet
}
```

Response:

```json
{
  "registry_id": "0x1234567890abcdef...",
  "tx_digest": "231EUwYN9iXw4fUDWjoqnV6zfc5zaCG96wX71KwzdfqM",
  "admin_address": "0x41d26f8218ba28e6ef35d58ddc937fc2e52706c2d0791cf1b3a03b229153f688"
}
```

**Features**:
- Creates a registry smart contract on Sui
- Extracts registry ID from RegistryCreatedEvent
- Uses distributed locking for transaction safety
- Supports multiple Sui networks (devnet, testnet, mainnet)

## Monitoring

### CloudWatch Logs

View logs in AWS Console or via CLI:

```bash
aws logs tail /aws/lambda/rust-lambda-function --follow
```

### Log Format

Logs include timestamp, level, module, and message:

```
2025-08-16 16:47:48.417 INFO  [lambda::handler] Incoming POST request to /add from 88.230.51.187
2025-08-16 16:47:48.419 INFO  [api] Processing add operation: a=2, b=3
```

## Recent Improvements

### Code Refactoring (August 2025)
- **Modular Sui crate structure**: Separated concerns into `chain.rs`, `add.rs`, and `registry.rs`
- **Eliminated code duplication**: Shared helper functions for RPC URLs, gas selection, and sender loading
- **Fixed timing bug in lock.rs**: Corrected elapsed time calculation preventing immediate timeouts
- **Improved error detection**: Fixed ConditionalCheckFailedException detection for proper retry logic

### New Features
- **Silvana Registry Creation**: Added support for creating registries on Sui blockchain
- **Event-based data extraction**: Registry ID extracted from RegistryCreatedEvent for reliability
- **String serialization for Move**: Proper BCS encoding of Move String type (struct with bytes field)
- **Transaction timing logs**: Added millisecond-precision timing for all blockchain operations
- **Automated backups**: Comprehensive backup strategy with PITR and scheduled backups
- **TypeScript client improvements**: Fixed type imports and added DOM.Iterable for Headers support

### Bug Fixes
- **Lock timing bug**: Fixed `Utc::now() - Utc::now()` always returning 0
- **gRPC read_mask**: Removed unsupported fields like `object_changes` and `effects`
- **String encoding**: Fixed Move String serialization as struct with bytes field
- **TypeScript compatibility**: Updated imports to match generated OpenAPI types

## Troubleshooting

### Build Issues

- **`cargo lambda` not found**: Install with `cargo install cargo-lambda`
- **OpenAPI generator errors**: Ensure Java is installed (required by openapi-generator)
- **Out of memory during build**: Increase ulimit with `ulimit -n 10240`

### Deployment Issues

- **Pulumi errors**: Check AWS credentials with `aws sts get-caller-identity`
- **Lambda timeout**: Increase timeout in `pulumi/index.ts`
- **Invalid runtime**: Ensure using `provided.al2023` for custom runtime

### Blockchain Issues

- **No transaction hash**: Check Sui environment variables are set
- **Transaction fails**: Verify Sui account has sufficient balance
- **Wrong network**: Ensure `SUI_CHAIN` matches your package deployment
- **Key locked error**: Check DynamoDB locks table for stuck locks
- **Concurrent transaction failures**: Verify key locking mechanism is working

### Storage Issues

- **KMS access denied**: Check Lambda IAM role has KMS permissions
- **DynamoDB errors**: Verify tables exist and Lambda has DynamoDB permissions
- **Keypair not found**: Check login_type and login match exactly
- **Encryption failures**: Ensure KMS key exists and is accessible

## Contributing

1. Fork the repository
2. Create a feature branch
3. Update OpenAPI spec if adding endpoints
4. Add tests for new functionality
5. Ensure `make lambda` builds successfully
6. Submit a pull request

## License

Apache 2.0

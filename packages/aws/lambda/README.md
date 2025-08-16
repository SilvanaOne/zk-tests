# AWS Lambda Rust Calculator with Sui Blockchain Integration

A serverless calculator API built with Rust, deployed on AWS Lambda, with optional Sui blockchain integration for mathematical operations.

## Architecture

- **AWS Lambda**: Serverless compute using ARM64 architecture with custom Rust runtime
- **API Gateway**: REST API endpoint for HTTP access
- **OpenAPI**: API-first development with automated code generation
- **Sui Blockchain**: On-chain computation for addition operations
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
   
   # or via npm
   npm install @openapitools/openapi-generator-cli -g
   ```

6. **AWS CLI** (configured with credentials)
   ```bash
   # macOS
   brew install awscli
   
   # Configure
   aws configure
   ```

### Optional Tools

- **jq** (for JSON formatting in tests)
- **curl** (for API testing)

## Project Structure

```
├── api/
│   └── openapi.yaml           # OpenAPI specification
├── crates/
│   ├── api/                   # API business logic
│   ├── api-generated/         # Auto-generated OpenAPI models
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
           $ref: '#/components/schemas/YourRequest'
         responses:
           '200':
             $ref: '#/components/schemas/YourResponse'
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
- `LOG_LEVEL`: Logging level (default: "info")
- `SUI_PACKAGE_ID`: Sui smart contract address (optional)
- `SUI_CHAIN`: Sui network (mainnet/testnet/devnet)
- `SUI_ADDRESS`: Sui wallet address
- `SUI_SECRET_KEY`: Sui private key (stored as Pulumi secret)

### Infrastructure Settings

Edit `pulumi/index.ts` to configure:
- Lambda memory size (default: 512MB)
- Lambda timeout (default: 30s)
- Architecture (default: ARM64)
- Log retention (default: 7 days)

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

## Contributing

1. Fork the repository
2. Create a feature branch
3. Update OpenAPI spec if adding endpoints
4. Add tests for new functionality
5. Ensure `make lambda` builds successfully
6. Submit a pull request

## License

Apache 2.0

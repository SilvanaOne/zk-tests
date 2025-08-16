# Lambda API TypeScript Client

TypeScript client library for the Silvana Lambda API, auto-generated from OpenAPI specification.

## Installation

```bash
npm install
```

## Generate Client Code

The client code is auto-generated from the OpenAPI specification:

```bash
npm run generate
# or
cd .. && make openapi-generate-ts
```

## Configuration

Create a `.env` file in the client directory:

```bash
cp .env.example .env
```

Edit `.env` and set your Lambda API endpoint:

```env
LAMBDA_API_URL=https://your-api-gateway-url.execute-api.us-east-1.amazonaws.com/stage
```

To get the API URL from a deployed stack:

```bash
cd ../pulumi && pulumi stack output functionUrl
```

## Usage

### Basic Example

```typescript
import { LambdaClient } from '@silvana/lambda-client';

const client = new LambdaClient('https://your-api-url.com');

// Add two numbers
const result = await client.add(10, 20);
console.log(result.result); // 30

// Generate or retrieve a Sui keypair
const keypair = await client.generateSuiKeypair('email', 'user@example.com');
console.log(keypair.address); // 0x...
```

### API Methods

#### Math Operations

```typescript
// Addition (may use Sui blockchain if configured)
const addResult = await client.add(5, 3);
// Returns: { result: 8, operation: 'add', txHash?: '0x...' }

// Multiplication (local computation)
const multiplyResult = await client.multiply(4, 7);
// Returns: { result: 28, operation: 'multiply' }
```

#### Sui Keypair Management

```typescript
// Generate or retrieve a keypair for a user
const keypair = await client.generateSuiKeypair('github', 'username');
// Returns: { address: '0x...' }

// Same credentials always return the same address
const keypair2 = await client.generateSuiKeypair('github', 'username');
console.log(keypair.address === keypair2.address); // true
```

## Testing

### Run All Tests

```bash
npm test
```

### Run Tests with Coverage

```bash
npm run test:coverage
```

### Run Manual Test Script

```bash
npm run dev
```

## Project Structure

```
client/
├── src/
│   ├── generated/      # Auto-generated OpenAPI client
│   └── index.ts        # Client wrapper with convenience methods
├── test/
│   ├── keypair.test.ts # Keypair generation tests
│   └── test.ts         # Manual test script
├── package.json
├── tsconfig.json
└── README.md
```

## Development

### Building

```bash
npm run build
```

This compiles TypeScript files to JavaScript in the `dist/` directory.

### Regenerating Client

When the OpenAPI specification changes:

1. Update the OpenAPI spec at `../api/openapi.yaml`
2. Regenerate the client:
   ```bash
   npm run generate
   ```
3. Update tests if needed

## API Features

- **Math Operations**: Add and multiply numbers with optional blockchain integration
- **Secure Keypair Storage**: Generate and retrieve Sui keypairs with KMS encryption
- **Distributed Locking**: Prevents concurrent key usage across Lambda instances
- **Type Safety**: Fully typed TypeScript client with auto-generated models

## Error Handling

The client uses native fetch and will throw errors for:
- Network failures
- Non-2xx HTTP responses
- Invalid responses

Example error handling:

```typescript
try {
  const result = await client.add(10, 20);
  console.log(result);
} catch (error) {
  console.error('API call failed:', error);
}
```

## License

Apache 2.0
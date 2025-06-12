# TEE Wallet Logging System

This document describes the comprehensive logging system implemented for the TEE Wallet application.

## Log Files Location

All logs are written to the `./logs` directory with the following files:

### Daily Rolling Log Files

1. **`tee-wallet.log.YYYY-MM-DD`** - Contains all application logs (INFO level and above)
2. **`tee-wallet-errors.log.YYYY-MM-DD`** - Contains only ERROR level logs
3. **`tee-wallet-access.log.YYYY-MM-DD`** - Contains access logs (requests, responses, logins)

The `.YYYY-MM-DD` suffix is automatically added by the daily rolling appender.

## Log Format

All logs are written in **JSON format** for easy parsing and analysis. Each log entry contains:

- `timestamp`: ISO 8601 timestamp
- `level`: Log level (INFO, ERROR, etc.)
- `fields`: Structured data specific to each event
- `target`: Module that generated the log

## Logged Events

### 1. Request/Response Logging

**All HTTP requests and responses are logged with:**

- IP address (supports X-Forwarded-For and X-Real-IP headers)
- HTTP method
- URI
- Status code
- Timestamp

**Example:**

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "INFO",
  "fields": {
    "event": "request_received",
    "method": "POST",
    "uri": "/login",
    "client_ip": "192.168.1.100",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

### 2. Login Events

**For successful logins (200 responses):**

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "INFO",
  "fields": {
    "event": "login_success",
    "client_ip": "192.168.1.100",
    "chain": "solana",
    "wallet": "phantom",
    "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

**For failed logins:**

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "ERROR",
  "fields": {
    "event": "login_error",
    "client_ip": "192.168.1.100",
    "chain": "solana",
    "wallet": "phantom",
    "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
    "error": "Invalid signature",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

### 3. Error Logging

**All Rocket framework errors are logged:**

- **404 Not Found**
- **500 Internal Server Error**
- **422 Unprocessable Entity**
- **400 Bad Request**

**Example:**

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "ERROR",
  "fields": {
    "event": "404_not_found",
    "uri": "/nonexistent",
    "method": "GET",
    "client_ip": "127.0.0.1",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

### 4. Application Error Logging

**Database errors, verification errors, and encryption errors:**

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "ERROR",
  "fields": {
    "event": "database_error",
    "operation": "get_kv",
    "error": "Connection failed",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

```json
{
  "timestamp": "2024-01-01T12:00:00.000Z",
  "level": "ERROR",
  "fields": {
    "event": "verification_error",
    "chain": "solana",
    "address": "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
    "error": "Invalid signature format",
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "target": "tee_wallet"
}
```

## Configuration

### Environment Variables

Set `RUST_LOG` environment variable to control logging levels:

```bash
# Show all logs
RUST_LOG=info cargo run

# Show only errors
RUST_LOG=error cargo run

# Show debug information (development)
RUST_LOG=debug cargo run
```

### Log Rotation

- **Daily rotation**: New files are created each day
- **Automatic cleanup**: Old log files remain for manual cleanup
- **Non-blocking**: Logging doesn't block application performance

## Development vs Production

### Development

- Logs are also output to console with pretty formatting
- Full debug information available

### Production

- Only file logging (no console output)
- JSON format for log aggregation systems
- Optimized for performance

## Log Analysis

Since logs are in JSON format, you can easily analyze them using tools like:

```bash
# Filter login successes
jq 'select(.fields.event == "login_success")' logs/tee-wallet.log.*

# Count errors by type
jq -r '.fields.event' logs/tee-wallet-errors.log.* | sort | uniq -c

# Find all requests from specific IP
jq 'select(.fields.client_ip == "192.168.1.100")' logs/tee-wallet-access.log.*
```

## Security Considerations

- **IP Address Logging**: Real client IPs are captured even behind proxies
- **No Sensitive Data**: Private keys, signatures, and messages are not logged
- **Error Details**: Full error messages are logged for debugging
- **Structured Format**: Easy to integrate with SIEM systems

## File Permissions

Ensure the `./logs` directory has appropriate permissions:

```bash
chmod 750 ./logs
chown app:app ./logs  # Replace with appropriate user/group
```

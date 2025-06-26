# TSA (Time Stamp Authority) Client

A Rust library for requesting and verifying RFC 3161 timestamps with certificate chain verification.

## Features

- **RFC 3161 Compliance**: Full support for Time Stamp Protocol as defined in RFC 3161
- **Certificate Verification**: Automatic verification of TSA certificate chains similar to AWS Nitro attestation
- **High Precision Timestamps**: Support for millisecond, microsecond, and higher precision timestamps
- **Comprehensive Response Parsing**: Detailed analysis of timestamp responses including accuracy information
- **Security Focused**: Built-in certificate chain validation and signature verification

## Certificate Verification

This implementation includes comprehensive certificate chain verification that:

- ✅ Validates certificate expiration dates
- ✅ Checks key usage extensions (digital signature for signer, certificate signing for CAs)
- ✅ Verifies basic constraints for CA certificates
- ✅ Validates issuer/subject chain relationships
- ⚠️ Certificate signature verification (commented out due to library limitations)
- ⚠️ Root certificate verification against trusted stores (not implemented)

The verification is modeled after the AWS Nitro attestation verification process found in the `attestation/` directory.

## Usage

```rust
use digicert_tsa::get_timestamp;

let data = b"Important document content";
let response = get_timestamp(data, "https://timestamp.digicert.com")?;

println!("Timestamp: {}", response.time_string);
println!("Certificate chain valid: {}", response.cert_verification.is_valid);
```

## Response Structure

The `TsaResponse` includes:

- `gen_time`: The timestamp from the TSA
- `time_string`: Human-readable timestamp
- `precision_info`: Details about timestamp precision
- `accuracy_info`: TSA-provided accuracy bounds
- `serial_number`: Unique timestamp serial number
- `cert_verification`: Certificate chain verification results

### Certificate Verification Result

```rust
pub struct CertVerificationResult {
    pub is_valid: bool,              // Overall verification result
    pub cert_count: usize,           // Number of certificates in chain
    pub error_message: Option<String>, // Error details if verification failed
    pub signer_cert_subject: Option<String>, // Subject DN of signing certificate
}
```

## Security Considerations

- This implementation performs basic certificate chain validation but does not verify against trusted root certificate stores
- Signature verification is commented out due to x509-parser library limitations
- For production use, consider implementing full signature verification using libraries like `ring` or `rustls`
- Always verify the TSA's root certificate against your trusted certificate store

## Example

See `examples/simple_timestamp.rs` for a complete example that demonstrates both timestamping and certificate verification.

```bash
cargo run --example simple_timestamp
```

## Dependencies

- `x509-parser`: Certificate parsing and basic validation
- `cms`: CMS/PKCS#7 message parsing
- `x509-tsp`: RFC 3161 timestamp protocol support
- `reqwest`: HTTP client for TSA communication

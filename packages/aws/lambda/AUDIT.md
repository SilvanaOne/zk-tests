## Serverless Backend Audit Report

Scope: `api/openapi.yaml`, Rust crates in `crates/`, infrastructure and permissions in `pulumi/index.ts`, and `Makefile`. Explicitly out of scope: `client/` and external Sui SDK code.

### Executive Summary

- The project exhibits strong modularization, uses AWS KMS envelope encryption for private keys, DynamoDB with TTL and PITR plus AWS Backup, and structured logging. OpenAPI drives model generation and there are basic unit tests.
- High-risk items: unauthenticated public access to sensitive endpoints via Lambda Function URL, secrets delivered as Lambda environment variables, and broad public CORS. Some IAM grants and resources appear unused.
- Key recommendations: enforce authentication/authorization, move sensitive material to Secrets Manager/SSM with KMS, enable KMS key rotation, restrict CORS, remove unused S3/permissions, and formalize request validation and rate limiting.

---

### Strengths

- Modular Rust workspace: distinct crates for Lambda handler, API, DB, KMS, and Sui integration; shared clients cached with `OnceLock` for cold-start efficiency.
- Cryptography/KMS: envelope encryption using KMS `GenerateDataKey` with AES-256-GCM; private keys stored encrypted; decryption limited to KMS key ID.
- State and concurrency: DynamoDB-based locking with TTL (`expires_at`) and conditional writes; explicit release and a Drop fallback.
- Resilience and backups: PITR enabled and scheduled AWS Backup daily/weekly/monthly plans for DynamoDB tables.
- Observability: structured logs via `tracing` with custom formatter; per-request and per-operation logging.
- OpenAPI-first: comprehensive schema for math and Sui registry operations; consistent response models.

---

### Findings and Recommendations

#### 1) Public Function URL with Sensitive Operations [High]

- `pulumi/index.ts` creates a Lambda Function URL with `authorizationType: "NONE"` and permissive `cors: { allowOrigins: ["*"], allowMethods: ["GET","POST"] }`.
- Endpoints like `/generate-sui-keypair`, `/sign-message`, and registry CRUD are accessible without auth when invoked via Function URL.

Recommendations:

- Remove the Function URL in production, or set `authorizationType: "AWS_IAM"` and front with an authenticated API Gateway or CloudFront + Lambda@Edge/JWT validation.
- Add WAF with IP throttling/bot control for the public edge if exposure is required.
- Restrict CORS (specific origins, methods) and consider pre-shared API keys or Cognito/JWT (prefer JWT) for all state-changing endpoints.
- Define OpenAPI `securitySchemes` (e.g., Bearer JWT) and mark secured operations accordingly; implement auth in the handler.

#### 2) Secrets in Lambda Environment Variables [High]

- Pulumi injects `SUI_SECRET_KEY` (and related) into Lambda env vars. Lambda encrypts env vars at rest but they are plaintext in-memory and visible to anyone with function read access. They may also leak via logs accidentally.

Recommendations:

- Store secrets in AWS Secrets Manager or SSM Parameter Store (SecureString) encrypted with KMS. Fetch at startup and cache; rotate via Secrets Manager rotation or CI.
- Restrict IAM so the function can read only the specific secret/parameter; avoid passing long-lived private keys via plain env vars.
- Review logs to ensure no secret values are printed. Avoid logging full request bodies that may contain sensitive data.

#### 3) Excessive Public CORS [High]

- CORS `*` on Function URL invites cross-origin calls.

Recommendations:

- Limit `allowOrigins` to trusted domains; remove `GET` if not needed.
- Prefer API Gateway v2 HTTP API with proper CORS, authorizers, throttling, and usage plans instead of Function URL for public APIs.

#### 4) Request Logging May Leak PII/Sensitive Data [Medium]

- `crates/lambda/src/handler.rs` logs the entire request body at debug: `debug!("Request body: {}", body_str);` This may include login identifiers and message bytes for signing.

Recommendations:

- Avoid logging raw bodies. Log minimal metadata (path, requestId). Redact fields like `login`, `message`, and any secrets.
- Add a structured redaction helper and unit tests to prevent regressions.

#### 5) KMS Key Rotation Disabled and Key Policy Clarity [Medium]

- `aws.kms.Key` lacks `enableKeyRotation: true`. Role permissions are granted via IAM, but no explicit KMS key policy for the Lambda role is defined.

Recommendations:

- Set `enableKeyRotation: true` on the KMS key. Consider explicit key policy statements granting the Lambda role `kms:Decrypt` and `kms:GenerateDataKey` to avoid relying solely on IAM.
- Tag the key with data classification; document rotation cadence.

#### 6) Unused S3 Bucket and Excess IAM [Medium]

- An S3 bucket `silvana-lambda` is created and IAM allows the Lambda to `s3:GetObject/PutObject` on it, but the function code is provided via `AssetArchive`, not S3.

Recommendations:

- Remove the S3 bucket and the related S3 IAM actions if not used. If you intend to upload artifacts to S3, switch to `BucketObject` and reference it from Lambda; otherwise keep infrastructure minimal.

#### 7) API Gateway (Classic) vs HTTP API v2 [Medium]

- Using `awsx.classic.apigateway.API` adds overhead and cost. Routes expose `/add`, `/multiply`, and a catch-all proxy; no request validation or throttling is configured.

Recommendations:

- Migrate to `aws.apigatewayv2.Api` (HTTP API). Add:
  - JWT or IAM authorizer
  - Usage plans and throttling limits
  - Schema-based request validation using OpenAPI (reject invalid payloads)

#### 8) OpenAPI Spec: Missing Security and Error Consistency [Medium]

- `api/openapi.yaml` defines operations but lacks `components.securitySchemes` and per-path `security` entries. Error responses for 401/403 are absent.

Recommendations:

- Add `securitySchemes` (Bearer JWT) and apply to all non-public endpoints. Include `401/403` responses and error payloads. Add tags and operation-level `x-rate-limit` annotations (for documentation) if useful.

#### 9) Generated Crate Edition Mismatch [Low]

- Workspace edition is `2024` (great), but `crates/api-generated/Cargo.toml` sets `edition = "2021"`.

Recommendations:

- Configure OpenAPI generator to emit 2024 edition or remove the edition override and inherit workspace edition.

#### 10) Logging Setup Uses `unsafe` Unnecessarily [Low]

- `crates/lambda/src/main.rs` wraps `std::env::set_var` in an `unsafe` block; this is not required and suggests risk where there is none.

Recommendations:

- Remove `unsafe` usage; regular `set_var` is safe.

#### 11) Locking Implementation Considerations [Low]

- Conditional write uses `condition_expression("attribute_not_exists(address)")` on a table with composite key (`address`, `chain`). For a given composite key, this is sufficient to ensure item creation is conditional on non-existence; TTL cleans up stale locks. Drop-based async release depends on an active runtime.

Recommendations:

- Prefer explicit `attribute_not_exists(address)` on the target item (current approach is acceptable). Continue to favor explicit release paths in code and keep TTL short. Consider adding CloudWatch alarms on lock contention rate.

#### 12) Public Resource Names and Retention [Low]

- S3 bucket name `silvana-lambda` is globally scoped and may collide; log retention is 7 days.

Recommendations:

- Use Pulumi auto-naming or suffix with stack/account/region. Increase log retention to 30–90 days per your compliance requirements.

#### 13) Error Messages May Leak Internals [Low]

- Some errors propagate internal blockchain or parsing messages to clients (e.g., `ApiError::Blockchain(format!(...))`).

Recommendations:

- Map internal errors to generic user messages; include detailed context only in logs.

---

### Code and Configuration Review Notes

- OpenAPI (`api/openapi.yaml`): schemas are coherent; int64 bounds approximate u64; consider adding examples for all registry endpoints and standardizing error enums. Add tagging (`tags:`) to group endpoints.
- Lambda handler (`crates/lambda/src/handler.rs`): robust handling of API Gateway vs Function URL events; base64 body decoding implemented; unify response path; add redaction for body logging.
- API crate (`crates/api/src/lib.rs`): good separation of async/sync; local add/multiply use `checked_*` with overflow checks; registry flows handle shared object versioning; consider unit/integration tests for registry endpoints, and add request validation beyond `serde_json::from_str` (e.g., bounds, string length).
- DB crate (`crates/db`): DynamoDB client reuse via `OnceLock`; secure storage uses binary primary key; values are bincode-encoded structs; include schema versioning field to allow future migrations; consider conditional writes on `store_keypair` to prevent overwrite.
- KMS crate (`crates/kms`): solid envelope encryption; enable key rotation; consider adding AAD in AES-GCM using contextual metadata (e.g., login key) to bind ciphertext.
- Sui crate (`crates/sui`): distributed locking around transactions is good; gas budget/price are static—consider adaptive strategies; avoid logging large event payloads; do not log secret material.
- Pulumi (`pulumi/index.ts`): IAM permissions are scoped to tables and key; remove redundant CloudWatch Logs actions duplicate of AWSLambdaBasicExecutionRole; consider per-stack resource names; make Function URL optional per stack config.
- Makefile: helpful dev ergonomics; `ulimit -n` portability is acceptable for macOS dev; consider adding `make test-security` or pre-deploy checks.

---

### Suggested Action Plan (Prioritized)

1. Security hardening

- Remove/lock down Function URL; switch to API Gateway HTTP API with JWT/IAM. Add WAF and throttling.
- Move `SUI_SECRET_KEY` and similar values to Secrets Manager/SSM; fetch at runtime; restrict IAM.
- Restrict CORS to trusted origins; remove unneeded methods.

2. KMS and IAM hygiene

- Enable KMS key rotation and add explicit key policy for the Lambda role.
- Remove unused S3 bucket and S3 permissions or start using it for artifacts.
- Remove redundant log permissions in the custom policy.

3. Observability and privacy

- Redact request bodies and sensitive fields; increase log retention to 30–90 days.
- Add metrics/alarms for lock contention and KMS/DynamoDB failures.

4. OpenAPI and validation

- Add `securitySchemes` and `security` requirements; extend responses with 401/403; add request validators on API Gateway.

5. Code quality

- Remove `unsafe` in `main.rs`; unify generated crate edition to Rust 2024; add schema versioning for stored values in DynamoDB; add integration tests for registry endpoints.

---

### Documentation Enhancements

- Add a Security section to `README.md` documenting:
  - Authentication model (JWT/IAM)
  - Secrets handling (Secrets Manager/SSM, KMS, rotation)
  - Key management policies (rotation enabled)
  - CORS policy and allowed origins
  - Rate limiting and abuse mitigation
- Add an Operations Runbook:
  - How to rotate keys/secrets safely
  - How to handle stuck locks and investigate contention
  - Backup restore procedures (already partially documented)
- Add an Architecture diagram (Lambda/API GW/KMS/DynamoDB/S3/Backup) and data flow diagram for keypair lifecycle.

---

### Appendix: Concrete Pulumi Changes (sketch)

```ts
// Enable KMS key rotation
const kmsKey = new aws.kms.Key("sui-keypair-encryption-key", {
  description: "KMS key for encrypting Sui private keys",
  keyUsage: "ENCRYPT_DECRYPT",
  customerMasterKeySpec: "SYMMETRIC_DEFAULT",
  enableKeyRotation: true,
});

// Make Function URL opt-in and protected
const useFunctionUrl = new pulumi.Config().getBoolean("useFunctionUrl");
if (useFunctionUrl) {
  new aws.lambda.FunctionUrl("lambda-function-url", {
    functionName: lambdaFunction.name,
    authorizationType: "AWS_IAM",
    cors: {
      allowOrigins: ["https://your.app"],
      allowMethods: ["POST"],
      allowHeaders: ["Content-Type"],
    },
  });
}

// Remove S3 bucket or actually use it; if unused, delete bucket resource and related IAM actions.
```

```rust
// Remove unsafe in main.rs
std::env::set_var("NO_COLOR", "1");
std::env::set_var("TERM", "dumb");
```

```toml
# crates/api-generated/Cargo.toml (prefer inheriting workspace edition 2024)
edition = "2024"
```

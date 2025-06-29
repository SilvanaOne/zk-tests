# Tonic network with kv

cargo run -- 127.0.0.1:50051 127.0.0.1:50052,127.0.0.1:50053
cargo run -- 127.0.0.1:50052 127.0.0.1:50051,127.0.0.1:50053
cargo run -- 127.0.0.1:50053 127.0.0.1:50051,127.0.0.1:50052

echo -n "Your text here" | base64

grpcurl -plaintext -import-path ./proto -proto kv.proto -d '{"key":"test-key", "value":"SGVsbG8gV29ybGQ="}' 127.0.0.1:50051 kv.Kv/Put

grpcurl -plaintext -import-path ./proto -proto kv.proto -d '{"key":"test-key"}' 127.0.0.1:50051 kv.Kv/Get

grpcurl -plaintext -import-path ./proto -proto kv.proto \
 -d '{"key":"greeting", "value":"SGVsbG8gV29ybGQ="}' \
 127.0.0.1:50051 kv.Kv/Put

grpcurl -plaintext -import-path ./proto -proto kv.proto \
 -d '{"key":"user:1", "value":"eyJuYW1lIjoiSm9obiJ9"}' \
 127.0.0.1:50051 kv.Kv/Put

## Benchmark

cargo run --release -- 127.0.0.1:50051
cargo test benchmark_kv_operations --release -- --nocapture

# Put a key-value pair

curl -X POST http://localhost:51051/api/kv \
 -H "Content-Type: application/json" \
 -d '{"key": "hello", "value": "world"}'

# Get a value

curl http://localhost:51051/api/kv/hello

# List all keys

curl http://localhost:51051/api/keys

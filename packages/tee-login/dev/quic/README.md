# Quic p2p network

cargo run -- 127.0.0.1:7001 127.0.0.1:7002,127.0.0.1:7003
cargo run -- 127.0.0.1:7002 127.0.0.1:7001,127.0.0.1:7003
cargo run -- 127.0.0.1:7003 127.0.0.1:7001,127.0.0.1:7002

curl -X PUT http://127.0.0.1:8001/kv/mykey \
 -H "Content-Type: application/json" \
 -d '{"value": "hello world"}'

curl http://127.0.0.1:8001/kv

curl -X PUT http://127.0.0.1:8001/kv/user:123 \
 -H "Content-Type: application/json" \
 -d '{"value": "John Doe"}'

curl -X PUT http://127.0.0.1:8001/kv/config:timeout \
 -H "Content-Type: application/json" \
 -d '{"value": "30s"}'

GET /kv - List all keys
GET /kv/{key} - Get value for key
PUT /kv/{key} - Set value for key (JSON body: {"value": "..."})

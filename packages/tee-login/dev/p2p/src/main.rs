use axum::{
    Router,
    extract::{Path, State},
    http::{Method, StatusCode},
    response::Json,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};
use kv::kv_server::{Kv, KvServer};
use kv::{GetReply, GetRequest, PutReply, PutRequest};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{sync::RwLock, task};
use tonic::{Request, Response, Status, transport::Server};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

pub mod kv {
    tonic::include_proto!("kv");
}

type Store = Arc<RwLock<HashMap<String, Vec<u8>>>>;

#[derive(Clone)]
struct KvSvc {
    store: Store,
}

#[tonic::async_trait]
impl Kv for KvSvc {
    async fn put(&self, req: Request<PutRequest>) -> Result<Response<PutReply>, Status> {
        let kv = req.into_inner();
        debug!("PUT key='{}' value_len={}", kv.key, kv.value.len());
        self.store.write().await.insert(kv.key, kv.value);
        Ok(Response::new(PutReply {}))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetReply>, Status> {
        let key = req.into_inner().key;
        let guard = self.store.read().await;
        let (value, found) = guard
            .get(&key)
            .map(|v| (v.clone(), true))
            .unwrap_or_default();
        debug!(
            "GET key='{}' found={} value_len={}",
            key,
            found,
            value.len()
        );
        Ok(Response::new(GetReply { value, found }))
    }
}

// REST API DTOs that auto-convert from/to protobuf
#[derive(Serialize, Deserialize)]
struct RestPutRequest {
    key: String,
    value: String, // We'll handle string values, encode binary as base64 if needed
}

#[derive(Serialize, Deserialize)]
struct RestPutResponse {
    success: bool,
}

#[derive(Serialize, Deserialize)]
struct RestGetResponse {
    value: Option<String>, // None if not found, Some(base64) for binary data
    found: bool,
}

// REST handlers that call the gRPC service internally
async fn rest_put(
    State(store): State<Store>,
    Json(payload): Json<RestPutRequest>,
) -> Result<Json<RestPutResponse>, StatusCode> {
    debug!("REST PUT key='{}' value='{}'", payload.key, payload.value);

    // Convert to protobuf and call gRPC service internally
    let kv_svc = KvSvc { store };
    let grpc_request = Request::new(PutRequest {
        key: payload.key,
        value: payload.value.into_bytes(),
    });

    match kv_svc.put(grpc_request).await {
        Ok(_) => Ok(Json(RestPutResponse { success: true })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn rest_get(
    State(store): State<Store>,
    Path(key): Path<String>,
) -> Result<Json<RestGetResponse>, StatusCode> {
    debug!("REST GET key='{}'", key);

    // Convert to protobuf and call gRPC service internally
    let kv_svc = KvSvc { store };
    let grpc_request = Request::new(GetRequest { key });

    match kv_svc.get(grpc_request).await {
        Ok(response) => {
            let reply = response.into_inner();
            let value = if reply.found {
                // Try to convert bytes to string, fallback to base64 for binary data
                match String::from_utf8(reply.value.clone()) {
                    Ok(string_val) => Some(string_val),
                    Err(_) => Some(general_purpose::STANDARD.encode(&reply.value)),
                }
            } else {
                None
            };

            Ok(Json(RestGetResponse {
                value,
                found: reply.found,
            }))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// List all keys (bonus REST endpoint not in gRPC)
async fn rest_list_keys(State(store): State<Store>) -> Json<Vec<String>> {
    let guard = store.read().await;
    let keys: Vec<String> = guard.keys().cloned().collect();
    Json(keys)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // read CLI flags: our bind addr + comma-separated peer list
    let mut args = std::env::args().skip(1);
    let addr: SocketAddr = args.next().expect("addr").parse()?;
    let peers: Vec<String> = args
        .next()
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    info!("Starting P2P node on {}", addr);
    info!("Connecting to {} peers: {:?}", peers.len(), peers);

    let store: Store = Arc::new(RwLock::new(HashMap::new()));
    let svc = KvSvc {
        store: store.clone(),
    };

    // Create CORS layer for browser requests
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .expose_headers(Any);

    // Create REST API routes (will run on separate port)
    let rest_api = Router::new()
        .route("/api/kv/:key", get(rest_get)) // GET /api/kv/{key}
        .route("/api/kv", post(rest_put)) // POST /api/kv
        .route("/api/keys", get(rest_list_keys)) // GET /api/keys
        .layer(cors)
        .with_state(store.clone());

    // Calculate REST API port (gRPC port + 1000)
    let rest_port = addr.port() + 1000;
    let rest_addr = SocketAddr::new(addr.ip(), rest_port);

    // Spawn REST API server in background
    task::spawn(async move {
        info!("REST API server listening on {}", rest_addr);
        let listener = tokio::net::TcpListener::bind(rest_addr).await.unwrap();
        axum::serve(listener, rest_api).await.unwrap();
    });

    // Build gRPC service with reflection and gRPC-Web support
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(include_bytes!("../proto/kv_descriptor.bin"))
        .build()?;

    // spawn one background task per peer
    for target in peers {
        let _store = store.clone();
        task::spawn(async move {
            info!("Attempting to connect to peer: {}", target);
            let mut client =
                match kv::kv_client::KvClient::connect(format!("http://{target}")).await {
                    Ok(client) => {
                        info!("Successfully connected to peer: {}", target);
                        client
                    }
                    Err(e) => {
                        error!("Failed to connect to peer {}: {}", target, e);
                        return;
                    }
                };

            // demo: periodically PUT our node-stamp so others can GET it
            loop {
                let key = format!("heartbeat:{}", target);
                let val = format!("{:?}", std::time::SystemTime::now()).into_bytes();

                match client
                    .put(PutRequest {
                        key: key.clone(),
                        value: val,
                    })
                    .await
                {
                    Ok(_) => debug!("Sent heartbeat to {}", target),
                    Err(e) => {
                        warn!("Failed to send heartbeat to {}: {}", target, e);
                        // Try to reconnect on next iteration
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }

            warn!("Connection to peer {} lost, task ending", target);
        });
    }

    info!("gRPC server listening on {} (gRPC + gRPC-Web)", addr);
    info!("REST API server listening on {}", rest_addr);

    // Start the gRPC server with gRPC-Web support
    Server::builder()
        .accept_http1(true) // Enable HTTP/1.1 for gRPC-Web
        .layer(tonic_web::GrpcWebLayer::new()) // Add gRPC-Web support
        .add_service(KvServer::new(svc))
        .add_service(reflection)
        .serve(addr)
        .await?;

    Ok(())
}

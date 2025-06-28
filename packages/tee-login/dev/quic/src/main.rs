//! quic-kvnet -- Put / Get mesh over QUIC using protobuf
//! -----------------------------------------------------
//! # run three terminals:
//! cargo run -- 127.0.0.1:7001 127.0.0.1:7002,127.0.0.1:7003
//! cargo run -- 127.0.0.1:7002 127.0.0.1:7001,127.0.0.1:7003
//! cargo run -- 127.0.0.1:7003 127.0.0.1:7001,127.0.0.1:7002

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, put},
};
use bytes::BytesMut;
use prost::Message;
use quinn::Endpoint;
use rcgen::generate_simple_self_signed;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::ServerName;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
    task,
};
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};

/// compiled from proto/kv.proto by prost_build in build.rs
pub mod kv {
    include!(concat!(env!("OUT_DIR"), "/kv.rs"));
}
use kv::envelope::Kind; // enum generated for the `oneof`

type Store = Arc<RwLock<HashMap<String, Vec<u8>>>>;

/// Custom certificate verifier that accepts any certificate
/// This is for demo purposes only - DO NOT use in production!
#[derive(Debug)]
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Self {
        Self
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    /* ------- initialize tracing and crypto ------------------------------- */
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Install default crypto provider for rustls
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Failed to install default crypto provider"))?;

    info!("Starting QUIC KV network node");
    warn!("⚠️  Using INSECURE certificate verification for demo purposes!");

    /* ------- parse CLI -------------------------------------------------- */
    let mut args = std::env::args().skip(1);
    let bind: SocketAddr = args
        .next()
        .context("first arg = bind addr (e.g. 127.0.0.1:7001)")?
        .parse()
        .context("failed to parse bind address")?;

    let peers: Vec<SocketAddr> = args
        .next()
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().context("failed to parse peer address"))
        .collect::<Result<Vec<_>>>()?;

    info!("Binding to: {}, connecting to peers: {:?}", bind, peers);

    /* ------- QUIC endpoint (server + default client config) ------------- */
    let endpoint = make_endpoint(bind).context("failed to create QUIC endpoint")?;
    info!("QUIC endpoint created successfully");

    /* ------- shared in-memory KV store ---------------------------------- */
    let store: Store = Arc::new(RwLock::new(HashMap::new()));

    /* ------- handle every inbound connection ---------------------------- */
    {
        let store_in = store.clone();
        let endpoint_clone = endpoint.clone();
        task::spawn(async move {
            info!("Starting connection accept loop");
            while let Some(incoming) = endpoint_clone.accept().await {
                let store = store_in.clone();
                task::spawn(async move {
                    match incoming.await {
                        Ok(connection) => {
                            let remote_addr = connection.remote_address();
                            debug!("Accepted connection from: {}", remote_addr);
                            if let Err(e) = handle_conn(connection, store).await {
                                error!("Connection error from {}: {:#}", remote_addr, e);
                            } else {
                                debug!("Connection from {} closed cleanly", remote_addr);
                            }
                        }
                        Err(e) => error!("Failed to establish connection: {:#}", e),
                    }
                });
            }
            warn!("Connection accept loop ended");
        });
    }

    /* ------- dial static peers ------------------------------------------ */
    for peer in peers {
        let ep = endpoint.clone();
        let _store_out = store.clone(); // Keep for future use
        task::spawn(async move {
            info!("Starting connection loop to peer: {}", peer);
            loop {
                match ep.connect(peer, "kvnet") {
                    Ok(connecting) => {
                        debug!("Attempting to connect to {}", peer);
                        match connecting.await {
                            Ok(conn) => {
                                info!("✓ Connected to {}", peer);
                                // send a heartbeat Put every 5 seconds
                                loop {
                                    match conn.open_bi().await {
                                        Ok((mut send, _recv)) => {
                                            let env = kv::Envelope {
                                                kind: Some(Kind::Put(kv::Put {
                                                    key: format!("heartbeat:{peer}"),
                                                    value: format!("{:?}", SystemTime::now())
                                                        .into_bytes(),
                                                })),
                                            };
                                            if let Err(e) = send_env(&mut send, &env).await {
                                                error!(
                                                    "Failed to send heartbeat to {}: {:#}",
                                                    peer, e
                                                );
                                                break;
                                            }
                                            debug!("Sent heartbeat to {}", peer);
                                            tokio::time::sleep(Duration::from_secs(5)).await;
                                        }
                                        Err(e) => {
                                            error!("Failed to open stream to {}: {:#}", peer, e);
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => error!("Handshake to {} failed: {:#}", peer, e),
                        }
                    }
                    Err(e) => error!("Connect() to {} failed: {:#}", peer, e),
                }
                debug!("Retrying connection to {} in 3 seconds", peer);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }

    /* ------- HTTP API server ------------------------------------------- */
    let http_port = bind.port() + 1000; // HTTP on port+1000 (e.g., 8001 for QUIC 7001)
    let http_addr = SocketAddr::new(bind.ip(), http_port);

    let app = Router::new()
        .route("/kv", get(http_list_keys))
        .route("/kv/:key", get(http_get_key))
        .route("/kv/:key", put(http_put_key))
        .layer(CorsLayer::permissive())
        .with_state(store.clone());

    task::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr)
            .await
            .expect("Failed to bind HTTP server");
        info!("🌐 HTTP API server listening on http://{}", http_addr);
        info!("API endpoints:");
        info!("  GET  http://{}/kv          - list all keys", http_addr);
        info!("  GET  http://{}/kv/{{key}}    - get value", http_addr);
        info!(
            "  PUT  http://{}/kv/{{key}}    - set value (JSON: {{\"value\": \"...\"}})",
            http_addr
        );

        axum::serve(listener, app)
            .await
            .expect("HTTP server failed");
    });

    info!("🚀 QUIC KV server listening on {}", bind);
    info!("Use RUST_LOG=debug for verbose output, RUST_LOG=info for normal output");

    // Wait forever
    futures::future::pending::<()>().await;
    Ok(())
}

/* ====================================================================== */
/* =====================-- HTTP API handlers --========================== */
/* ====================================================================== */

// GET /kv/{key}
async fn http_get_key(
    Path(key): Path<String>,
    State(store): State<Store>,
) -> Result<Json<Value>, StatusCode> {
    let val = store.read().await.get(&key).cloned();
    match val {
        Some(value) => {
            let value_str = String::from_utf8_lossy(&value);
            Ok(Json(json!({
                "key": key,
                "value": value_str,
                "found": true
            })))
        }
        None => Ok(Json(json!({
            "key": key,
            "value": null,
            "found": false
        }))),
    }
}

// PUT /kv/{key} with JSON body: {"value": "..."}
async fn http_put_key(
    Path(key): Path<String>,
    State(store): State<Store>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let value = payload
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    store
        .write()
        .await
        .insert(key.clone(), value.as_bytes().to_vec());

    Ok(Json(json!({
        "key": key,
        "value": value,
        "status": "stored"
    })))
}

// GET /kv - list all keys
async fn http_list_keys(State(store): State<Store>) -> Json<Value> {
    let keys: Vec<String> = store.read().await.keys().cloned().collect();
    Json(json!({
        "keys": keys,
        "count": keys.len()
    }))
}

/* ====================================================================== */
/* =====================-- helpers & handlers --========================= */
/* ====================================================================== */

async fn handle_conn(connection: quinn::Connection, store: Store) -> Result<()> {
    let remote_addr = connection.remote_address();
    debug!("Handling connection from {}", remote_addr);

    while let Ok((send, recv)) = connection.accept_bi().await {
        let store = store.clone();
        let remote_addr = remote_addr;
        task::spawn(async move {
            let (mut recv, mut send) = (recv, send);

            let result: Result<()> = async {
                let env = recv_env(&mut recv)
                    .await
                    .context("failed to receive envelope")?;

                match env.kind {
                    Some(Kind::Put(kv::Put { key, value })) => {
                        debug!("PUT request from {}: key={}", remote_addr, key);
                        store.write().await.insert(key.clone(), value);
                        let ack = kv::Envelope {
                            kind: Some(Kind::Reply(kv::GetReply {
                                value: vec![],
                                found: true,
                            })),
                        };
                        send_env(&mut send, &ack)
                            .await
                            .context("failed to send PUT ack")?;
                        debug!("PUT acknowledged for key: {}", key);
                    }
                    Some(Kind::Get(kv::Get { key })) => {
                        debug!("GET request from {}: key={}", remote_addr, key);
                        let val = store.read().await.get(&key).cloned();
                        let found = val.is_some();
                        let reply = kv::Envelope {
                            kind: Some(Kind::Reply(kv::GetReply {
                                value: val.unwrap_or_default(),
                                found,
                            })),
                        };
                        send_env(&mut send, &reply)
                            .await
                            .context("failed to send GET reply")?;
                        debug!("GET replied for key: {} (found: {})", key, found);
                    }
                    Some(Kind::Reply(_)) => {
                        debug!("Received unexpected reply from {}", remote_addr);
                    }
                    None => {
                        warn!("Received envelope with no kind from {}", remote_addr);
                    }
                }
                Ok(())
            }
            .await;

            if let Err(e) = result {
                error!("Stream error from {}: {:#}", remote_addr, e);
            }
        });
    }
    debug!("Connection from {} closed", remote_addr);
    Ok(())
}

/* ----- build server+client QUIC endpoint ------------------------------ */
fn make_endpoint(bind: SocketAddr) -> Result<Endpoint> {
    debug!("Creating QUIC endpoint for {}", bind);

    // self-signed cert so nodes accept each other w/out files on disk
    let cert = generate_simple_self_signed([bind.to_string()])
        .context("failed to generate self-signed certificate")?;
    let priv_key = PrivateKeyDer::try_from(cert.serialize_private_key_der())
        .map_err(|e| anyhow::anyhow!("Private key conversion failed: {}", e))?;
    let cert_der = CertificateDer::from(
        cert.serialize_der()
            .context("failed to serialize certificate")?,
    );

    debug!("Generated self-signed certificate");

    let mut server_cfg =
        quinn::ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.clone_key())
            .context("failed to create server config")?;
    Arc::get_mut(&mut server_cfg.transport)
        .unwrap()
        .max_concurrent_bidi_streams(64_u32.into());

    // For demo purposes, we'll use a custom verifier that accepts any certificate
    // This allows nodes with different self-signed certs to connect to each other
    let client_crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification::new()))
        .with_no_client_auth();

    let client_cfg = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
            .context("failed to create QUIC client config")?,
    ));

    debug!("Created TLS configuration");

    let mut ep = Endpoint::server(server_cfg, bind).context("failed to create QUIC endpoint")?;
    ep.set_default_client_config(client_cfg);

    info!("QUIC endpoint created successfully on {}", bind);
    Ok(ep)
}

/* ----- protobuf framing helpers --------------------------------------- */
async fn recv_env<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<kv::Envelope> {
    let len = r
        .read_u32()
        .await
        .context("failed to read message length")? as usize;

    if len > 1_000_000 {
        // 1MB limit
        return Err(anyhow::anyhow!("Message too large: {} bytes", len));
    }

    let mut buf = vec![0; len];
    r.read_exact(&mut buf)
        .await
        .context("failed to read message body")?;

    kv::Envelope::decode(&*buf).map_err(|e| anyhow::anyhow!("failed to decode protobuf: {}", e))
}

async fn send_env<W: AsyncWriteExt + Unpin>(w: &mut W, env: &kv::Envelope) -> Result<()> {
    let mut buf = BytesMut::with_capacity(env.encoded_len());
    env.encode(&mut buf)
        .map_err(|e| anyhow::anyhow!("failed to encode protobuf: {}", e))?;

    w.write_u32(buf.len() as u32)
        .await
        .context("failed to write message length")?;
    w.write_all(&buf)
        .await
        .context("failed to write message body")?;
    w.flush().await.context("failed to flush message")?;
    Ok(())
}

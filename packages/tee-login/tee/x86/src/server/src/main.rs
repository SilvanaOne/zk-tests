// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Logging Usage:
// Set RUST_LOG environment variable to control log levels:
// - RUST_LOG=error    (only errors)
// - RUST_LOG=warn     (warnings and errors)
// - RUST_LOG=info     (info, warnings, and errors) [DEFAULT]
// - RUST_LOG=debug    (debug, info, warnings, and errors)
// - RUST_LOG=trace    (all log levels)
// - RUST_LOG=server=debug,hyper=warn (module-specific levels)

use anyhow::Result;
use axum::{Router, routing::get, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use rustls::{ServerConfig, pki_types::CertificateDer};
use rustls_pemfile;
use server::AppState;
use server::app::{login, ping};
use server::common::{get_attestation, health_check};
use server::dynamodb::DynamoDB;
use server::keys::Keys;
use server::stats::stats;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber to see logs
    // Default to "info" level if RUST_LOG is not set, but support all levels
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default configuration: info level for the application, warn for dependencies
        tracing_subscriber::EnvFilter::new("info,hyper=warn,h2=warn,tower=warn,reqwest=warn")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true) // Include the module path in logs
        .with_thread_ids(true) // Include thread IDs
        .with_level(true) // Include log level
        .with_file(true) // Include file name
        .with_line_number(true) // Include line number
        .init();

    info!("Starting Silvana TEE Login server...");
    debug!("Tracing initialized with all log levels: error, warn, info, debug, trace");
    let keys = Keys::new();
    info!(
        "Generated ephemeral key pair for session: {:?}",
        keys.to_addresses()
    );

    // Initialize the database
    info!("Loading environment configuration...");
    let table = std::env::var("DB").expect("DB environment variable not set");
    let key_id = std::env::var("KMS_KEY_ID").expect("KMS_KEY_ID environment variable not set");
    let aws_region = std::env::var("AWS_REGION").expect("AWS_REGION environment variable not set");
    info!("AWS Region: {}", aws_region);
    let aws_access_key_id =
        std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID environment variable not set");
    debug!("AWS Access Key ID: {}", aws_access_key_id);
    let aws_secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .expect("AWS_SECRET_ACCESS_KEY environment variable not set");
    debug!("AWS Secret Access Key: {}", aws_secret_access_key);
    let acm_certificate = std::env::var("ACM_CERTIFICATE")
        .expect("ACM_CERTIFICATE environment variable not set");
    debug!("ACM Certificate: {}", acm_certificate);
    warn!("Sensitive credentials loaded - ensure secure environment");

    info!("Initializing database...");
    let db_store = match DynamoDB::new(table, key_id).await {
        Ok(db) => {
            info!("Database initialized successfully");
            db
        }
        Err(e) => {
            error!("Failed to initialize DynamoDB: {}", e);
            std::process::exit(1);
        }
    };

    info!("Creating app state...");
    let state = Arc::new(AppState { keys, db_store });

    info!("Setting up CORS...");
    // Define your own restricted CORS policy here if needed.
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    info!("Setting up routes...");
    let app = Router::new()
        .route("/", get(hello))
        .route("/get_attestation", get(get_attestation))
        .route("/login", post(login))
        .route("/ping", post(ping))
        .route("/health_check", get(health_check))
        .route("/stats", get(stats))
        .with_state(state)
        .layer(cors);

    // Configure TLS with ACM certificate
    info!("Configuring HTTPS with ACM certificate...");
    let tls_config = match configure_tls(&acm_certificate).await {
        Ok(config) => {
            info!("TLS configuration successful");
            config
        }
        Err(e) => {
            error!("Failed to configure TLS: {}", e);
            error!("Make sure ACM for Nitro Enclaves is properly configured on the parent instance");
            std::process::exit(1);
        }
    };

    // Bind to both HTTP (3000) and HTTPS (443) ports
    info!("Starting HTTPS server on port 443...");
    let https_addr = "0.0.0.0:443";
    info!("HTTPS server listening on {}", https_addr);
    warn!("HTTPS server bound to 0.0.0.0:443 - accessible from all network interfaces");
    
    // Also start HTTP server on port 3000 for health checks and internal access
    let http_app = app.clone();
    let http_handle = tokio::spawn(async move {
        info!("Starting HTTP server on port 3000 for health checks...");
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await
            .map_err(|e| anyhow::anyhow!("Failed to bind HTTP server: {}", e))?;
        info!("HTTP server listening on 0.0.0.0:3000");
        axum::serve(listener, http_app.into_make_service())
            .await
            .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))
    });

    // Start HTTPS server
    let https_future = axum_server::bind_rustls(https_addr.parse()?, tls_config)
        .serve(app.into_make_service());

    // Wait for either server to complete (or fail)
    tokio::select! {
        result = https_future => {
            result.map_err(|e| anyhow::anyhow!("HTTPS server error: {}", e))
        }
        result = http_handle => {
            match result {
                Ok(inner_result) => inner_result,
                Err(e) => Err(anyhow::anyhow!("HTTP server task error: {}", e))
            }
        }
    }
}

async fn hello() -> &'static str {
    "Hello!"
}

/// Configure TLS using ACM certificate and private key files
async fn configure_tls(acm_certificate_id: &str) -> Result<RustlsConfig> {
    info!("Configuring TLS with ACM certificate: {}", acm_certificate_id);
    
    // Load certificate chain from the file provided by ACM for Nitro Enclaves
    let cert_chain_path = "/opt/aws/acm/cert_chain.pem";
    info!("Loading certificate chain from: {}", cert_chain_path);
    
    let cert_file = File::open(cert_chain_path)
        .map_err(|e| anyhow::anyhow!("Failed to open certificate chain file {}: {}", cert_chain_path, e))?;
    let mut cert_reader = BufReader::new(cert_file);
    
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<CertificateDer>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate chain: {}", e))?;
    
    if cert_chain.is_empty() {
        return Err(anyhow::anyhow!("No certificates found in certificate chain file"));
    }
    
    info!("Loaded {} certificates from chain", cert_chain.len());
    
    // Load private key from the file provided by ACM for Nitro Enclaves
    let private_key_path = "/opt/aws/acm/private_key.pem";
    info!("Loading private key from: {}", private_key_path);
    
    // Try to read private key using the generic private key parser
    let private_key = {
        let key_file = File::open(private_key_path)
            .map_err(|e| anyhow::anyhow!("Failed to open private key file {}: {}", private_key_path, e))?;
        let mut key_reader = BufReader::new(key_file);
        
        rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| anyhow::anyhow!("Failed to parse private key: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No private key found in key file"))?
    };
    
    info!("Successfully loaded private key");
    
    // Build rustls server configuration
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|e| anyhow::anyhow!("Failed to build TLS configuration: {}", e))?;
    
    info!("TLS configuration completed successfully");
    Ok(RustlsConfig::from_config(Arc::new(config)))
}

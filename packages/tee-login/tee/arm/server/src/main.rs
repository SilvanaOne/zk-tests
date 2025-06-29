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
use server::AppState;
use server::app::{login, ping};
use server::common::{get_attestation, health_check};
use server::coordination::get_request_data;
use server::dynamodb::DynamoDB;
use server::keys::Keys;
use server::stats::stats;
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
    let request_data = match get_request_data().await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to get request data: {}", e);
            return Err(anyhow::anyhow!("Failed to get request data: {}", e));
        }
    };
    info!("Request data: {:?}", request_data);
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

    info!("Binding to port 3000...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let addr = listener.local_addr().unwrap();
    info!("Server listening on {}", addr);
    warn!("Server bound to 0.0.0.0 - accessible from all network interfaces");
    debug!("Ready to accept incoming connections");
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))
}

async fn hello() -> &'static str {
    "Hello!"
}

// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use axum::{Router, routing::get, routing::post};
use fastcrypto::{ed25519::Ed25519KeyPair, traits::KeyPair};
use silvana_tee_server::AppState;
use silvana_tee_server::app::login;
use silvana_tee_server::common::{get_attestation, health_check};
use silvana_tee_server::dynamodb::DynamoDB;
use silvana_tee_server::stats::stats;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    let eph_kp = Ed25519KeyPair::generate(&mut rand::thread_rng());

    info!("Starting Silvana TEE Login server...");

    // Initialize the database
    let table = std::env::var("DB").expect("DB environment variable not set");
    let key_name =
        std::env::var("KMS_KEY_NAME").expect("KMS_KEY_NAME environment variable not set");
    let db_store = match DynamoDB::new(table, key_name).await {
        Ok(db) => {
            info!("Database initialized successfully");
            db
        }
        Err(e) => {
            error!("Failed to initialize DynamoDB: {}", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(AppState { eph_kp, db_store });

    // Define your own restricted CORS policy here if needed.
    let cors = CorsLayer::new().allow_methods(Any).allow_headers(Any);

    let app = Router::new()
        .route("/", get(ping))
        .route("/get_attestation", get(get_attestation))
        .route("/login", post(login))
        .route("/health_check", get(health_check))
        .route("/stats", get(stats))
        .with_state(state)
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))
}

async fn ping() -> &'static str {
    "Pong!"
}

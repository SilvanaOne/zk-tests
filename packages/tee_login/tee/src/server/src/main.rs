// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use axum::{Router, routing::get, routing::post};
use fastcrypto::{ed25519::Ed25519KeyPair, traits::KeyPair};
use server::AppState;
use server::app::login;
use server::common::{get_attestation, health_check};
use server::dynamodb::DynamoDB;
use server::stats::stats;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Silvana TEE Login server...");
    let eph_kp = Ed25519KeyPair::generate(&mut rand::thread_rng());

    // Initialize the database
    let table = std::env::var("DB").expect("DB environment variable not set");
    let key_name =
        std::env::var("KMS_KEY_NAME").expect("KMS_KEY_NAME environment variable not set");
    let aws_region = std::env::var("AWS_REGION").expect("AWS_REGION environment variable not set");
    println!("AWS Region: {}", aws_region);
    let aws_access_key_id =
        std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID environment variable not set");
    println!("AWS Access Key ID: {}", aws_access_key_id);
    let aws_secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .expect("AWS_SECRET_ACCESS_KEY environment variable not set");
    println!("AWS Secret Access Key: {}", aws_secret_access_key);

    println!("Initializing database...");
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

    println!("Creating app state...");
    let state = Arc::new(AppState { eph_kp, db_store });

    println!("Setting up CORS...");
    // Define your own restricted CORS policy here if needed.
    let cors = CorsLayer::new().allow_methods(Any).allow_headers(Any);

    println!("Setting up routes...");
    let app = Router::new()
        .route("/", get(ping))
        .route("/get_attestation", get(get_attestation))
        .route("/login", post(login))
        .route("/health_check", get(health_check))
        .route("/stats", get(stats))
        .with_state(state)
        .layer(cors);

    println!("Binding to port 3000...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on {}", listener.local_addr().unwrap());
    info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))
}

async fn ping() -> &'static str {
    "Pong!"
}

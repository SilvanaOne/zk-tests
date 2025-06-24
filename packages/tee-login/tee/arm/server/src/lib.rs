// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Json;
use dynamodb::DynamoDB;
use keys::Keys;
use serde_json::json;

pub mod app;
pub mod attestation;
pub mod auth;
pub mod common;
pub mod db;
pub mod dynamodb;
pub mod encrypt;
pub mod hash;
pub mod keys;
pub mod kms;
pub mod logger;
pub mod login;
pub mod seed;
pub mod shamir;
pub mod stats;
pub mod time;

/// App state, at minimum needs to maintain the ephemeral keypair.  
pub struct AppState {
    /// Ephemeral keypair on boot
    pub keys: Keys,
    /// Database
    pub db_store: DynamoDB,
}

/// Implement IntoResponse for EnclaveError.
impl IntoResponse for EnclaveError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            EnclaveError::GenericError(e) => (StatusCode::BAD_REQUEST, e),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

/// Enclave errors enum.
#[derive(Debug)]
pub enum EnclaveError {
    GenericError(String),
}

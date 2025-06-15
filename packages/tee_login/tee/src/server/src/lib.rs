// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use dynamodb::DynamoDB;
use fastcrypto::ed25519::Ed25519KeyPair;
use serde_json::json;

pub mod app;
pub mod attestation;
pub mod common;
pub mod db;
pub mod dynamodb;
pub mod encrypt;
pub mod hash;
pub mod kms;
pub mod logger;
pub mod login;
pub mod seed;
pub mod shamir;
pub mod solana;
pub mod stats;
pub mod sui;

/// App state, at minimum needs to maintain the ephemeral keypair.  
pub struct AppState {
    /// Ephemeral keypair on boot
    pub eph_kp: Ed25519KeyPair,
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

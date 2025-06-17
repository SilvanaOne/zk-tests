// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::AppState;
use crate::EnclaveError;
use crate::common::IntentMessage;
use crate::common::{IntentScope, ProcessDataRequest, ProcessedDataResponse, to_signed_response};
use crate::login::{LoginRequest, LoginResponse, process_login};
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
//use serde_json::Value;
use std::sync::Arc;
use tracing::info;

/// Inner type T for IntentMessage<T>
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartStats {
    pub cpu_cores: u64,
    pub memory: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StartResponse {
    pub memo: String,
    pub time_started: u64,
    pub stats: StartStats,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProcessDataRequest<LoginRequest>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<LoginResponse>>>, EnclaveError> {
    info!("Login endpoint called");
    let login_response = process_login(request.payload, &state.db_store).await;
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;
    Ok(Json(to_signed_response(
        &state.keys.sui_keypair,
        login_response,
        current_timestamp,
        IntentScope::Login,
    )))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PingRequest {
    pub memo: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PingResponse {
    pub memo: String,
    pub timestamp: u64,
}

pub async fn ping(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProcessDataRequest<PingRequest>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<PingResponse>>>, EnclaveError> {
    info!("Ping endpoint called");
    info!("Ping request: {:?}", request.payload.memo);
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;
    info!("Current timestamp: {:?}", current_timestamp);
    let ping_response = PingResponse {
        memo: "pong".to_string(),
        timestamp: current_timestamp,
    };
    info!("Ping response: {:?}", ping_response);

    Ok(Json(to_signed_response(
        &state.keys.sui_keypair,
        ping_response,
        current_timestamp,
        IntentScope::Ping,
    )))
}

pub fn get_worker_stats() -> StartStats {
    let cpu_cores = num_cpus::get() as u64;
    let mem_info = sys_info::mem_info();
    info!("mem_info: {:?}", mem_info);
    let memory = sys_info::mem_info().map(|info| info.total).unwrap_or(0);

    StartStats { cpu_cores, memory }
}

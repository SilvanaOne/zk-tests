// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::AppState;
use crate::EnclaveError;
use crate::common::IntentMessage;
use crate::common::{IntentScope, ProcessedDataResponse, to_signed_response};
use crate::time::get_enclave_time;
use axum::Json;
use axum::extract::State;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
//use serde_json::Value;
use std::sync::Arc;
use tracing::info;

/// Inner type T for IntentMessage<T>
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stats {
    pub cpu_cores: u64,
    pub memory: u64,
    pub available_memory: u64,
    pub free_memory: u64,
    pub used_memory: u64,
    pub timestamp: String,
    pub current_timestamp: u64,
    pub enclave_timestamp: Option<u64>,
    pub timestamp_drift: Option<i128>,
    pub enclave_timestamp_error: Option<String>,
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<Stats>>>, EnclaveError> {
    info!("Getting worker stats");
    let stats = get_worker_stats()?;
    info!("Stats: {:?}", stats);
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;
    Ok(Json(to_signed_response(
        &state.keys.sui_keypair,
        stats,
        current_timestamp,
        IntentScope::Stats,
    )))
}

pub fn get_worker_stats() -> Result<Stats, EnclaveError> {
    let cpu_cores = num_cpus::get() as u64;
    let mem_info = sys_info::mem_info();
    info!("mem_info: {:?}", mem_info);
    let memory = sys_info::mem_info().map(|info| info.total).unwrap_or(0);
    let available_memory = sys_info::mem_info().map(|info| info.avail).unwrap_or(0);
    let free_memory = sys_info::mem_info().map(|info| info.free).unwrap_or(0);
    let used_memory = memory.saturating_sub(available_memory);

    let start = Utc::now();
    let (enclave_timestamp, enclave_timestamp_error) = match get_enclave_time() {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let now = Utc::now();
    let duration = now.signed_duration_since(start);
    info!("VSOCK timestamp call duration: {:?}", duration);
    // format as RFC3339 (ISO-8601) with exactly 3 fractional digits (milliseconds)
    let timestamp = now.to_rfc3339_opts(SecondsFormat::Millis, false);
    let current_timestamp = now.timestamp_millis() as u64;
    let timestamp_drift = match enclave_timestamp {
        Some(enclave_timestamp) => Some(enclave_timestamp as i128 - current_timestamp as i128),
        None => None,
    };

    Ok(Stats {
        cpu_cores,
        memory,
        available_memory,
        free_memory,
        used_memory,
        timestamp,
        current_timestamp,
        enclave_timestamp,
        timestamp_drift,
        enclave_timestamp_error,
    })
}

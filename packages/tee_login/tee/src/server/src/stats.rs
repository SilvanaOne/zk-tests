// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::AppState;
use crate::EnclaveError;
use crate::common::IntentMessage;
use crate::common::{IntentScope, ProcessedDataResponse, to_signed_response};
use axum::Json;
use axum::extract::State;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
//use serde_json::Value;
use std::sync::Arc;

/// Inner type T for IntentMessage<T>
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stats {
    pub cpu_cores: u64,
    pub memory: u64,
    pub timestamp: String,
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<Stats>>>, EnclaveError> {
    println!("Getting worker stats");
    let stats = get_worker_stats()?;
    println!("Stats: {:?}", stats);
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;
    println!("Current timestamp: {}", current_timestamp);
    Ok(Json(to_signed_response(
        &state.eph_kp,
        stats,
        current_timestamp,
        IntentScope::Stats,
    )))
}

pub fn get_worker_stats() -> Result<Stats, EnclaveError> {
    let cpu_cores = num_cpus::get() as u64;
    let mem_info = sys_info::mem_info();
    println!("mem_info: {:?}", mem_info);
    let memory = sys_info::mem_info().map(|info| info.total).unwrap_or(0);

    let now = Utc::now();
    // format as RFC3339 (ISO-8601) with exactly 3 fractional digits (milliseconds)
    let timestamp = now.to_rfc3339_opts(SecondsFormat::Millis, false);

    Ok(Stats {
        cpu_cores,
        memory,
        timestamp,
    })
}

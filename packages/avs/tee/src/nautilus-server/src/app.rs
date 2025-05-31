// Copyright (c), Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::AppState;
use crate::EnclaveError;
use crate::agent::start_agent;
use crate::common::IntentMessage;
use crate::common::{IntentScope, ProcessDataRequest, ProcessedDataResponse, to_signed_response};
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
//use serde_json::Value;
use std::sync::Arc;

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

/// Inner type T for ProcessDataRequest<T>
#[derive(Debug, Serialize, Deserialize)]
pub struct StartRequest {
    pub memo: String,
}

pub async fn process_data(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProcessDataRequest<StartRequest>>,
) -> Result<Json<ProcessedDataResponse<IntentMessage<StartResponse>>>, EnclaveError> {
    // let url = format!(
    //     "https://api.weatherapi.com/v1/current.json?key={}&q={}",
    //     state.api_key, request.payload.location
    // );
    // let response = reqwest::get(url.clone()).await.map_err(|e| {
    //     EnclaveError::GenericError(format!("Failed to get weather response: {}", e))
    // })?;
    // let json = response.json::<Value>().await.map_err(|e| {
    //     EnclaveError::GenericError(format!("Failed to parse weather response: {}", e))
    // })?;
    // let location = json["location"]["name"].as_str().unwrap_or("Unknown");
    // let temperature = json["current"]["temp_c"].as_f64().unwrap_or(0.0) as u64;
    // let last_updated_epoch = json["current"]["last_updated_epoch"].as_u64().unwrap_or(0);
    // let last_updated_timestamp_ms = last_updated_epoch * 1000_u64;
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EnclaveError::GenericError(format!("Failed to get current timestamp: {}", e)))?
        .as_millis() as u64;

    // // 1 hour in milliseconds = 60 * 60 * 1000 = 3_600_000
    // if last_updated_timestamp_ms + 3_600_000 < current_timestamp {
    //     return Err(EnclaveError::GenericError(
    //         "Weather API timestamp is too old".to_string(),
    //     ));
    // }
    let start_stats = get_worker_stats();
    let key = state.api_key.clone();
    tokio::spawn(async move {
        if let Err(e) = start_agent(&key).await {
            eprintln!("Agent error: {}", e);
        }
    });

    Ok(Json(to_signed_response(
        &state.eph_kp,
        StartResponse {
            memo: request.payload.memo.clone(),
            time_started: current_timestamp,
            stats: start_stats,
        },
        current_timestamp,
        IntentScope::Start,
    )))
}

pub fn get_worker_stats() -> StartStats {
    let cpu_cores = num_cpus::get() as u64;
    let mem_info = sys_info::mem_info();
    println!("mem_info: {:?}", mem_info);
    let memory = sys_info::mem_info().map(|info| info.total).unwrap_or(0);

    StartStats { cpu_cores, memory }
}
